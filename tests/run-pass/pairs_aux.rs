// ignore-test Not a test. Used by other tests

#![allow(dead_code)]  // Tests don't ordinarily use every feature and every accessor.

use std::rc::Rc;
use cell_gc::Heap;

gc_heap_type! {
    #[derive(Clone, Debug)]
    pub struct Pair / PairRef / PairStorage <'h> {
        head / set_head: Value<'h>,
        tail / set_tail: Value<'h>
    }
}

gc_heap_type! {
    #[derive(Clone, Debug, PartialEq)]
    pub enum Value / ValueStorage <'h> {
        Null,
        Int(i32),
        Str(Rc<String>),  // <-- equality is by value
        Pair(PairRef<'h>)  // <-- equality is by pointer
    }
}

/// Helper function to avoid having to write out `Pair` literals all over the place.
pub fn alloc_pair<'h>(heap: &mut Heap<'h>, head: Value<'h>, tail: Value<'h>) -> PairRef<'h> {
    heap.alloc(Pair { head: head, tail: tail })
}

/// Allocate a pair with the values `(null, null)`.
pub fn alloc_null_pair<'h>(heap: &mut Heap<'h>) -> PairRef<'h> {
    alloc_pair(heap, Value::Null, Value::Null)
}
