#[macro_export]
macro_rules! include_notice {
    () => {
        include_str!(concat!(env!("OUT_DIR"), concat!("/notalawyer")));
    };
}
