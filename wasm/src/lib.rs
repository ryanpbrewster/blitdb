extern "C" {
    fn host_log(code: i32);
}

#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    unsafe { host_log(a * a + b * b); }
    a + b
}
