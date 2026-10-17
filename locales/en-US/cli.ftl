# SteleOFD English (US) · Command line output

cli-pages = Pages: { $count }
cli-page-size = Page size: { $w } x { $h } mm
cli-version = Version: { $version }
cli-embedded-fonts = Embedded fonts: { $count }
cli-signatures = Signatures: { $count }
cli-no-pages = The document declares no pages
cli-wrote = Wrote { $path }
cli-rendered-page = Rendered page { $page } at { $dpi } dpi
cli-converted = Converted { $count ->
    [one] { $count } page
   *[other] { $count } pages
} to { $format }
cli-search-hit = page { $page }: { $snippet }
cli-audit-clean = No archival issues found
cli-audit-finding = { $code } [{ $severity }] { $location }: { $detail }
cli-print-plan = { $pages ->
    [one] { $pages } page
   *[other] { $pages } pages
} on { $sheets ->
    [one] { $sheets } sheet
   *[other] { $sheets } sheets
}, { $paper } at { $dpi } dpi
cli-sign-entry = { $loc }: { $status }
cli-sign-none = The document declares no signatures
cli-error = error: { $message }
