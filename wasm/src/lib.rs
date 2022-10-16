extern "C" {
    fn host_log(msg: *const u8, len: usize) -> i32;
    fn host_get(key: *const u8, key_len: usize, value: *mut u8, value_len: usize) -> i32;
    fn host_set(key: *const u8, key_len: usize, value: *const u8, value_len: usize) -> i32;
}

#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    log(b"reading 'hello'");
    let mut value = Vec::with_capacity(1024);
    get(b"hello", &mut value);
    log(format!("hello={:?}", &value).as_bytes());
    value.push(b'o');
    set(b"hello", &value);
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

fn get(key: &[u8], value: &mut Vec<u8>) {
    unsafe {
        let n = host_get(
            key.as_ptr(),
            key.len(),
            value.as_mut_ptr(),
            value.capacity(),
        );
        value.set_len(n as usize);
    };
}
