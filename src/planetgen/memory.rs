//! Mesure de la mémoire allouée (tests uniquement).
//!
//! Un allocateur qui compte les octets vivants alloués par le fil courant : les tests tournent en
//! parallèle, chacun ne voit que ses propres allocations.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static LIVE: Cell<isize> = const { Cell::new(0) };
}

fn add(n: isize) {
    let _ = LIVE.try_with(|c| c.set(c.get() + n));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        add(layout.size() as isize);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        add(-(layout.size() as isize));
        System.dealloc(ptr, layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        add(layout.size() as isize);
        System.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        add(new_size as isize - layout.size() as isize);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Octets alloués (et pas encore libérés) par ce fil.
pub fn live_bytes() -> isize {
    LIVE.with(|c| c.get())
}

/// Octets restés alloués après `f` (le résultat de `f` est gardé pendant la mesure).
pub fn retained_by<T>(f: impl FnOnce() -> T) -> (T, isize) {
    let before = live_bytes();
    let value = f();
    (value, live_bytes() - before)
}
