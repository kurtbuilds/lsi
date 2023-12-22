use std::alloc::{alloc, Layout, LayoutError};
use std::hash::BuildHasher;
use std::ptr::{addr_of, addr_of_mut, NonNull};
use std::sync::RwLock;
use hashbrown::hash_map::DefaultHashBuilder;
use hashbrown::HashSet;

/// The primary type of this crate.
/// Internally, it is a pointer to a leaked [`InternedData`] struct, which itself
/// is a representation of a string.
/// Because it is a pointer, it is `Copy`, and equality checking is a single instruction.
#[derive(Copy, Clone, Debug)]
pub struct Istr(NonNull<*const u8>);

assert_eq_size!(Istr, usize);
assert_eq_size!(Option<Istr>, usize);

unsafe impl Send for Istr {}

unsafe impl Sync for Istr {}

impl Istr {
    pub fn new(s: &str) -> Self {
        if s.is_empty() {
            return EMPTY_FAST_STR;
        }
        GLOBAL_TABLE.get_or_intern(s)
    }

    pub fn as_str(&self) -> &'static str {
        if self.0 == EMPTY_FAST_STR.0 {
            return "";
        }
        unsafe {
            let ptr = self.0;
            let len = addr_of!(*ptr.as_ptr()).read() as usize;
            // let slice_ptr = ptr.as_ptr().add(size_of::<usize>());
            let slice_ptr = ptr.as_ptr().add(1) as *const u8;
            // let slice_ptr = addr_of!(*(ptr.as_ptr().add(1)));
            let slice = std::slice::from_raw_parts(slice_ptr, len);
            std::str::from_utf8_unchecked(slice)
        }
    }

    pub fn len(&self) -> usize {
        if self.0 == EMPTY_FAST_STR.0 {
            return 0;
        }
        unsafe {
            let ptr = self.0;
            let len = addr_of!(*ptr.as_ptr()).read() as usize;
            len
        }
    }
}

impl PartialEq for Istr {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<&str> for Istr {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl Eq for Istr {}

impl Into<String> for Istr {
    fn into(self) -> String {
        self.as_str().to_owned()
    }
}

impl AsRef<str> for Istr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::hash::Hash for Istr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

#[derive(Debug)]
pub struct InternTable(RwLock<HashSet<Istr>>);

impl InternTable {
    pub fn get_or_intern(&self, s: &str) -> Istr {
        let mut lock = self.0.write().unwrap();
        let hasher = lock.hasher().clone();
        let hash = hasher.hash_one(s);
        let map = lock.raw_table_mut();
        if let Some(fast_str) = map.get(hash, |&(q, _)| q.as_str() == s) {
            return fast_str.0;
        }
        let fast_str = InternedData::construct(s);
        let inserted = map.insert_entry(hash, (fast_str, ()), |&(x, _)| hasher.hash_one(x));
        inserted.0
    }

    pub fn len(&self) -> usize {
        self.0.read().unwrap().len()
    }
}

pub static GLOBAL_TABLE: InternTable = InternTable(RwLock::new(HashSet::with_hasher(unsafe {
    std::mem::transmute::<_, DefaultHashBuilder>(())
})));

const EMPTY_FAST_STR: Istr = Istr(unsafe {
    // we're okay doing this because if the pointer pointed to the end
    // of memory, we'd be OOM anyway.
    NonNull::new_unchecked(usize::MAX as *mut *const u8)
});


/// This is the data that gets interned by the library.
#[repr(C)]
pub struct InternedData {
    len: usize,
    data: [u8],
}

// implementation taken from https://www.reddit.com/r/rust/comments/mq3kqe/comment/gue0du1/?utm_source=reddit&utm_medium=web2x&context=3
impl InternedData {
    pub fn construct(s: &str) -> Istr {
        let interned = InternedData::new(s);
        let leaked = Box::leak(interned);
        let ptr = leaked as *const InternedData;
        Istr(unsafe {
            NonNull::new_unchecked(ptr as *mut *const u8)
        })
    }

    pub fn new(s: &str) -> Box<Self> {
        let ptr = unsafe { Self::alloc_self(s.len()) };
        Self::initialize_self(ptr, s);
        let b = unsafe { Box::from_raw(ptr) };
        b
    }

    pub(crate) fn layout_of(n: usize) -> Result<Layout, LayoutError> {
        let (layout, _) = Layout::new::<usize>()
            .extend(Layout::array::<u8>(n)?)?;
        let layout = layout.pad_to_align();
        Ok(layout)
    }

    pub(crate) unsafe fn alloc_self(n: usize) -> *mut Self {
        // Find the layout with a helper function.
        let layout = Self::layout_of(n).unwrap();
        // Make a heap allocation.
        let ptr = alloc(layout);
        // Construct a fat pointer by making a fake slice.
        // The first argument is the pointer, the second argument is the metadata.
        // In this case, its just the length of the slice.
        let ptr = std::ptr::slice_from_raw_parts_mut(ptr, n) as *mut InternedData;
        // Transmute the slice into the real fat pointer type.
        //let ptr = std::mem::transmute::<_, *mut Form<T, H>>(ptr);
        ptr
    }

    pub fn initialize_self(ptr: *mut Self, s: &str) {
        // Initialize its fields one-by-one.
        unsafe {
            addr_of_mut!((*ptr).len).write(s.len());
        }
        unsafe {
            let slice_ptr = addr_of_mut!((*ptr).data) as *mut u8;
            std::ptr::copy_nonoverlapping(s.as_ptr(), slice_ptr, s.len());
        }
    }
}

#[macro_export]
macro_rules! assert_eq_size {
    ($x:ty, $($xs:ty),+ $(,)?) => {
        const _: fn() = || {
            $(let _ = core::mem::transmute::<$x, $xs>;)+
        };
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let s = Istr::new("Hello");
        assert_eq!(s, "Hello");
    }

    #[test]
    fn test_new_same_address() {
        let s = Istr::new("Hello");
        assert_eq!(s, "Hello");
        let t = Istr::new("Hello");
        assert_eq!(s.0, t.0);
    }
}
