#![allow(dead_code)]

extern "C" {
    fn host_get(key: i32) -> i32;
    fn host_set(key: i32, value: i32) -> i32;
}

#[no_mangle]
pub unsafe extern "C" fn exec() -> i32 {
    host_get(42)
}


















































unsafe fn get_and_increment(key: i32) -> i32 {
    let v = host_get(key);
    host_set(key, v + 1)
}

unsafe fn collatz(key: i32) -> i32 {
    let v = host_get(key);
    let next = if v % 2 == 0 { v / 2 } else { 3 * v + 1 };
    host_set(key, next)
}

unsafe fn next_prime(key: i32) -> i32 {
    let mut v = host_get(key);
    loop {
        v += 1;
        if is_prime(v) {
            return host_set(key, v);
        }
    }
}

fn is_prime(n: i32) -> bool {
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
        i += 2;
    }
    true
}

#[cfg(test)]
mod test {
    use crate::is_prime;

    #[test]
    fn is_prime_smoke_test() {
        let primes: Vec<i32> = (1..20).filter(|&i| is_prime(i)).collect();
        assert_eq!(primes, [2, 3, 5, 7, 11, 13, 17, 19]);
    }
}
