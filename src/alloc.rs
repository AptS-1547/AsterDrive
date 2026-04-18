//! 堆分配跟踪与统计 helper。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

/// 当前堆分配字节数
pub static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
/// 历史峰值
pub static PEAK: AtomicUsize = AtomicUsize::new(0);

pub struct TrackingAlloc;

#[inline]
fn record_alloc(size: usize) {
    let current = ALLOCATED.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(current, Ordering::Relaxed);
}

#[inline]
fn record_dealloc(size: usize) {
    ALLOCATED.fetch_sub(size, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            record_alloc(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        record_dealloc(layout.size());
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            match new_size.cmp(&layout.size()) {
                std::cmp::Ordering::Greater => record_alloc(new_size - layout.size()),
                std::cmp::Ordering::Less => record_dealloc(layout.size() - new_size),
                std::cmp::Ordering::Equal => {}
            }
        }
        new_ptr
    }
}

/// 获取当前分配量和峰值（MB）
pub fn stats() -> (f64, f64) {
    let allocated = ALLOCATED.load(Ordering::Relaxed) as f64 / 1_048_576.0;
    let peak = PEAK.load(Ordering::Relaxed) as f64 / 1_048_576.0;
    (allocated, peak)
}
