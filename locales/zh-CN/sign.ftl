# SteleOFD 简体中文 · 签章模块
# 注意复数形式：中文无复数变化，但保留 Fluent 选择结构以便其他语言复用同一键。

sign-valid = 有效
sign-invalid = 无效
sign-unverified = 无法验证
sign-expired = 证书已过期
sign-integrity-ok = 文档未被篡改
sign-tampered = 文档内容已被修改

# 签名数量（复数）
sign-count = { $count ->
    [0] 无签名
    [one] { $count } 个签名
   *[other] { $count } 个签名
}

sign-signer = 签章人：{ $name }
sign-signed-at = 签署时间：{ $time }
sign-algorithm = 算法：{ $algo }
sign-issuer = 颁发机构：{ $issuer }
sign-export-report = 导出验签报告
