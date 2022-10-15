extern "C" {
    fn host_log(code: i32);
    fn host_increment(code: i32) -> i32;
}

#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    unsafe { host_log(a * a + b * b); }
    let mut i = 0;
    while unsafe { host_increment(i) } > 1 {
        i += 1;
    }
    a + b + i
}
