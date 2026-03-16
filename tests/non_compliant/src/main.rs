// PATH002: `use` must be before `mod` or other items
use std::collections::HashMap;
mod stuff;

// FUNC003: Function alias
fn original_fn(a: i32) -> i32 { a + 1 }
fn wrapper_fn(a: i32) -> i32 { original_fn(a) }

// SAFE001 / SAFE002: Panic in non-test code
fn throw_errors() {
    let _val = Some("ok").unwrap();
    panic!("Should not be allowed here!");
}

// SAFE003: unsafe block without SAFETY comment
fn bad_unsafe() {
    unsafe {
        let _x = 1;
    }
}

// DOC001: Missing pub doc
pub struct UndocumentedStruct {
    pub field: i32,
}

// PATH001: Excessively long path
fn long_namespace() {
    let _map = std::collections::hash_map::DefaultHasher::new();
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}