//! Command implementations behind the `steleofd` binary.
//! Every entry point takes parsed arguments plus a localization context and
//! returns a rendered report, so the logic runs under test without a process.

#![warn(missing_docs)]

pub mod cli;

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use steleofd_archive::Severity;
use steleofd_core::Document;
use steleofd_error::{FontError, FormatError, OfdError};
use steleofd_font::FontSource;
use steleofd_i18n::format::LocalizedFormat;
use steleofd_i18n::{I18n, Locale, MessageId};
use steleofd_io::FileSource;
use steleofd_print::{PaperSize, PrintSettings};
use steleofd_render::RenderOptions;
use steleofd_search::SearchOptions;
use steleofd_sign::VerifyStatus;

use crate::cli::{Cli, Command, FormatArg, PaperArg};

/// What a command has to say, and whether the process should exit nonzero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Text ready to print, one line per finding.
    pub text: String,
    /// False when the command ran but its outcome is a failure.
    pub ok: bool,
}

impl Report {
    /// A successful report.
    pub fn ok(text: String) -> Self {
        Self { text, ok: true }
    }

    /// A report that should turn the exit code nonzero.
    pub fn failed(text: String) -> Self {
        Self { text, ok: false }
    }
}

/// Run one parsed command line.
pub fn run(cli: &Cli, i18n: &I18n) -> Result<Report> {
    match &cli.command {
        Command::Info(args) => info(&args.path, i18n),
        Command::Render(args) => render(
            &args.path,
            args.page,
            args.dpi,
            args.output.as_deref(),
            i18n,
        ),
        Command::Convert(args) => convert(
            &args.path,
            args.format,
            args.page,
            args.dpi,
            args.output.as_deref(),
            i18n,
        ),
        Command::Verify(args) => verify(&args.path, i18n),
        Command::Search(args) => search(
            &args.path,
            &args.needle,
            args.case_sensitive,
            args.whole_word,
            i18n,
        ),
        Command::Audit(args) => audit(&args.path, i18n),
        Command::Print(args) => print(&args.path, args.paper, args.dpi, args.copies, i18n),
    }
}

/// Turn a failure into a printable report.
/// Structured engine errors get a localized reason; anything else prints as-is.
pub fn error_report(err: &anyhow::Error, i18n: &I18n) -> Report {
    let message = match err.downcast_ref::<OfdError>() {
        Some(ofd) => localized_error(ofd, i18n),
        None => err.to_string(),
    };
    Report::failed(i18n.text_with(MessageId::CliError, [("message", message.into())]))
}

/// Open a document from a path.
fn open(path: &Path) -> Result<Document> {
    let source = FileSource::open(path)?;
    Ok(Document::open(&source)?)
}

/// Parse a command line, for tests and embedders.
/// The program name is supplied, so callers list arguments only.
pub fn parse(args: &[&str]) -> std::result::Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("steleofd").chain(args.iter().copied()))
}

/// Localization context for a parsed command line.
/// An explicit `--locale` wins; otherwise the system locale is negotiated.
pub fn i18n_for(cli: &Cli) -> I18n {
    match &cli.locale {
        Some(tag) => I18n::with_locale(Locale::new(tag)),
        None => I18n::with_system_locale(),
    }
}

fn info(path: &Path, i18n: &I18n) -> Result<Report> {
    let mut doc = open(path)?;
    let fmt = LocalizedFormat::new(i18n.locale().clone());
    let page_box = doc.model().common_data.page_area.physical_box;
    let embedded = doc
        .fonts()
        .registry()
        .iter()
        .filter(|entry| matches!(entry.source, FontSource::Embedded { .. }))
        .count();
    let mut lines = vec![
        i18n.text_with(MessageId::CliVersion, [("version", doc.version().into())]),
        i18n.text_with(
            MessageId::CliPages,
            [("count", fmt.integer(doc.page_count() as i64).into())],
        ),
        i18n.text_with(
            MessageId::CliPageSize,
            [
                ("w", fmt.decimal(page_box.w, 1).into()),
                ("h", fmt.decimal(page_box.h, 1).into()),
            ],
        ),
        i18n.text_with(
            MessageId::CliEmbeddedFonts,
            [("count", fmt.integer(embedded as i64).into())],
        ),
    ];
    if doc.page_count() == 0 {
        lines.push(i18n.text(MessageId::CliNoPages));
    }
    let signatures = doc.signatures().map(|list| list.len()).unwrap_or(0);
    lines.push(i18n.text_with(
        MessageId::CliSignatures,
        [("count", fmt.integer(signatures as i64).into())],
    ));
    Ok(Report::ok(lines.join("\n")))
}

fn render(
    path: &Path,
    page: usize,
    dpi: f64,
    output: Option<&Path>,
    i18n: &I18n,
) -> Result<Report> {
    let doc = open(path)?;
    let out = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(format!("page-{page}.png")));
    let png = doc.to_png(
        page,
        &RenderOptions {
            dpi,
            ..Default::default()
        },
    )?;
    std::fs::write(&out, png)?;
    let fmt = LocalizedFormat::new(i18n.locale().clone());
    let text = [
        i18n.text_with(
            MessageId::CliRenderedPage,
            [
                ("page", fmt.integer(page as i64).into()),
                ("dpi", fmt.decimal(dpi, 0).into()),
            ],
        ),
        i18n.text_with(
            MessageId::CliWrote,
            [("path", out.display().to_string().into())],
        ),
    ]
    .join("\n");
    Ok(Report::ok(text))
}

fn convert(
    path: &Path,
    format: FormatArg,
    page: Option<usize>,
    dpi: f64,
    output: Option<&Path>,
    i18n: &I18n,
) -> Result<Report> {
    let doc = open(path)?;
    let dir = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".to_string());
    let pages = doc.page_count();
    let indices: Vec<usize> = match page {
        Some(index) => vec![index],
        None => (0..pages).collect(),
    };

    let (count, wrote) = match format {
        FormatArg::Pdf => {
            let out = dir.join(format!("{stem}.pdf"));
            std::fs::write(&out, doc.to_pdf()?)?;
            (pages, Some(out))
        }
        FormatArg::Png => {
            let options = RenderOptions {
                dpi,
                ..Default::default()
            };
            for index in &indices {
                let out = dir.join(format!("{stem}-{index}.png"));
                std::fs::write(&out, doc.to_png(*index, &options)?)?;
            }
            (indices.len(), None)
        }
        FormatArg::Svg => {
            for index in &indices {
                let out = dir.join(format!("{stem}-{index}.svg"));
                std::fs::write(&out, doc.to_svg(*index)?)?;
            }
            (indices.len(), None)
        }
    };

    let mut lines = vec![i18n.text_with(
        MessageId::CliConverted,
        [
            ("count", (count as i64).to_string().into()),
            ("format", format.label().into()),
        ],
    )];
    if let Some(out) = wrote {
        lines.push(i18n.text_with(
            MessageId::CliWrote,
            [("path", out.display().to_string().into())],
        ));
    }
    Ok(Report::ok(lines.join("\n")))
}

fn verify(path: &Path, i18n: &I18n) -> Result<Report> {
    let mut doc = open(path)?;
    let refs = match doc.signatures() {
        Err(OfdError::Format(FormatError::MissingSignatures)) => {
            return Ok(Report::failed(i18n.text(MessageId::CliSignNone)));
        }
        result => result?,
    };
    let statuses = doc.verify_signatures()?;
    let lines: Vec<String> = refs
        .iter()
        .zip(&statuses)
        .map(|(sig, status)| {
            i18n.text_with(
                MessageId::CliSignEntry,
                [
                    ("loc", sig.base_loc.clone().into()),
                    ("status", verify_label(status, i18n).into()),
                ],
            )
        })
        .collect();
    let ok = statuses.iter().all(|s| matches!(s, VerifyStatus::Valid));
    Ok(Report {
        text: lines.join("\n"),
        ok,
    })
}

fn search(
    path: &Path,
    needle: &str,
    case_sensitive: bool,
    whole_word: bool,
    i18n: &I18n,
) -> Result<Report> {
    let doc = open(path)?;
    let options = SearchOptions {
        case_sensitive,
        whole_word,
        max_hits: None,
    };
    let fmt = LocalizedFormat::new(i18n.locale().clone());
    let lines: Vec<String> = doc
        .search(needle, options)
        .iter()
        .map(|hit| {
            i18n.text_with(
                MessageId::CliSearchHit,
                [
                    ("page", fmt.integer(hit.page as i64).into()),
                    ("snippet", hit.snippet.clone().into()),
                ],
            )
        })
        .collect();
    Ok(Report::ok(lines.join("\n")))
}

fn audit(path: &Path, i18n: &I18n) -> Result<Report> {
    let mut doc = open(path)?;
    let report = doc.audit()?;
    if report.findings.is_empty() {
        return Ok(Report::ok(i18n.text(MessageId::CliAuditClean)));
    }
    let lines: Vec<String> = report
        .findings
        .iter()
        .map(|finding| {
            i18n.text_with(
                MessageId::CliAuditFinding,
                [
                    ("code", finding.code.into()),
                    ("severity", finding.severity.label().into()),
                    ("location", finding.location.clone().into()),
                    ("detail", finding.detail.clone().into()),
                ],
            )
        })
        .collect();
    let ok = !report
        .findings
        .iter()
        .any(|f| matches!(f.severity, Severity::Error));
    Ok(Report {
        text: lines.join("\n"),
        ok,
    })
}

fn print(path: &Path, paper: PaperArg, dpi: f64, copies: usize, i18n: &I18n) -> Result<Report> {
    let doc = open(path)?;
    let settings = PrintSettings {
        paper: paper_size(paper),
        dpi,
        copies: copies.max(1),
        ..Default::default()
    };
    let job = doc.print_job(&settings)?;
    // Rasterize too: a plan that cannot produce bits is not a print job.
    let raster = doc.print_render(&settings)?;
    let sheets = job
        .pages
        .iter()
        .map(|planned| planned.sheet + 1)
        .max()
        .unwrap_or(0);
    let fmt = LocalizedFormat::new(i18n.locale().clone());
    let text = i18n.text_with(
        MessageId::CliPrintPlan,
        [
            ("pages", fmt.integer(job.pages.len() as i64).into()),
            ("sheets", fmt.integer(sheets as i64).into()),
            ("paper", paper.label().into()),
            ("dpi", fmt.decimal(dpi, 0).into()),
        ],
    );
    debug_assert_eq!(raster.len(), job.pages.len());
    Ok(Report::ok(text))
}

fn paper_size(paper: PaperArg) -> PaperSize {
    match paper {
        PaperArg::Document => PaperSize::Document,
        PaperArg::A3 => PaperSize::A3,
        PaperArg::A4 => PaperSize::A4,
        PaperArg::A5 => PaperSize::A5,
        PaperArg::B5 => PaperSize::B5,
        PaperArg::Letter => PaperSize::Letter,
        PaperArg::Legal => PaperSize::Legal,
    }
}

fn verify_label(status: &VerifyStatus, i18n: &I18n) -> String {
    match status {
        VerifyStatus::Valid => i18n.text(MessageId::SignValid),
        VerifyStatus::Tampered => i18n.text(MessageId::SignInvalid),
    }
}

/// Localized reason for a structured engine error.
fn localized_error(err: &OfdError, i18n: &I18n) -> String {
    let fmt = LocalizedFormat::new(i18n.locale().clone());
    match err {
        OfdError::Io(e) => i18n.text_with(MessageId::ErrIo, [("detail", e.to_string().into())]),
        OfdError::Package(e) => i18n.text_with(
            MessageId::ErrPackageCorrupted,
            [("reason", e.to_string().into())],
        ),
        OfdError::Xml(e) => i18n.text_with(
            MessageId::ErrPackageCorrupted,
            [("reason", e.to_string().into())],
        ),
        OfdError::Format(format) => match format {
            FormatError::NotOfd => i18n.text(MessageId::ErrNotOfd),
            FormatError::PageOutOfRange { index, total } => i18n.text_with(
                MessageId::ErrPageOutOfRange,
                [
                    ("index", fmt.integer(*index as i64).into()),
                    ("total", fmt.integer(*total as i64).into()),
                ],
            ),
            FormatError::DanglingReference { kind, id } => i18n.text_with(
                MessageId::ErrDanglingReference,
                [("kind", (*kind).into()), ("id", id.clone().into())],
            ),
            FormatError::MaxDepthExceeded(depth) => i18n.text_with(
                MessageId::ErrResourceLimit,
                [("detail", fmt.integer(*depth as i64).into())],
            ),
            _ => i18n.text_with(
                MessageId::ErrUnsupported,
                [("feature", err.to_string().into())],
            ),
        },
        OfdError::Font(FontError::Missing { family }) => i18n.text_with(
            MessageId::ErrFontMissing,
            [("family", family.clone().into())],
        ),
        OfdError::Font(_) => i18n.text_with(
            MessageId::ErrUnsupported,
            [("feature", err.to_string().into())],
        ),
        OfdError::ResourceLimit(detail) => i18n.text_with(
            MessageId::ErrResourceLimit,
            [("detail", detail.clone().into())],
        ),
        OfdError::PasswordRequired => i18n.text(MessageId::ErrPasswordRequired),
        _ => i18n.text_with(
            MessageId::ErrUnsupported,
            [("feature", err.to_string().into())],
        ),
    }
}
