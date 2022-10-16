extern "C" {
    fn host_log(msg: *const u8, len: usize);
    fn host_set(key: *const u8, key_len: usize, value: *const u8, value_len: usize) -> i32;
}

#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    log(b"setting hello=world");
    set(b"hello", b"world");
    a + b
}

fn log(msg: &[u8]) {
    unsafe {
        host_log(msg.as_ptr(), msg.len());
    }
}

fn set(key: &[u8], value: &[u8]) {
    unsafe {
        host_set(key.as_ptr(), key.len(), value.as_ptr(), value.len());
    }
}
