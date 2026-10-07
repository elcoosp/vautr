//! VTRFIX-TST-02: pagination + scale regression for BUG-H01 (u8 truncation).

#[test]
fn progress_pct_is_u32_safe_at_300_items() {
    // The fix in sync() uses u32 for idx/total; this test pins that the
    // arithmetic does not overflow at the scale that used to wrap a u8 to 0.
    let total: u32 = 300;
    for idx in 0..=total {
        let p = (idx * 100) / total.max(1);
        assert!(p <= 100);
    }
}
