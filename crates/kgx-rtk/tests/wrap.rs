use kgx_rtk::wrap::run_with_rtk;

#[test]
fn falls_back_to_raw_when_rtk_off() {
    std::env::set_var("KGX_RTK", "off");
    let mut c = std::process::Command::new("echo");
    c.arg("hello world");
    let out = run_with_rtk(&mut c).unwrap();
    assert!(out.stdout.contains("hello world"));
    assert_eq!(out.raw_bytes, out.compressed_bytes);
}
