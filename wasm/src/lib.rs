extern "C" {
    fn host_log(msg: *const u8, len: usize) -> i32;
    fn host_get(key: *const u8, key_len: usize, value: *mut u8, value_len: usize) -> i32;
    fn host_set(key: *const u8, key_len: usize, value: *const u8, value_len: usize) -> i32;
}

#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    for i in 2.. {
        if !is_prime(i) {
            continue;
        }
        let key = format!("hello-{}", i);
        log(format!("reading key={}", key).as_bytes());
        let cur = get(key.as_bytes());
        if cur.is_empty() {
            set(key.as_bytes(), b"here");
            break;
        }
    }
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

fn get(key: &[u8]) -> Vec<u8> {
    let mut value = Vec::with_capacity(1_024);
    unsafe {
        let len = host_get(
            key.as_ptr(),
            key.len(),
            value.as_mut_ptr(),
            value.capacity(),
        );
        value.set_len(len as usize);
    }
    log(format!("read {:?}={:?}", key, value).as_bytes());
    value
}

fn is_prime(n: u32) -> bool {
    if n < 2 {
        return false;
    }
    if n % 2 == 0 {
        return n == 2;
    }
    let mut i = 3;
    while i * i <= n {
        if n % i == 0 {
            return false;
        }
        println!("{} % {} > 0", n, i);
        i += 2;
    }
    true
}

#[cfg(test)]
mod test {
    use crate::is_prime;

    #[test]
    fn is_prime_smoke_test() {
        let primes: Vec<u32> = (1..20).filter(|&i| is_prime(i)).collect();
        assert_eq!(primes, [2, 3, 5, 7, 11, 13, 17, 19]);
    }
}
