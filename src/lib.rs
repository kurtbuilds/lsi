use std::alloc::{alloc, Layout, LayoutError};
use std::hash::{BuildHasher, BuildHasherDefault};
use hashbrown::{HashSet, hash_map::DefaultHashBuilder};
use std::ptr::{addr_of, addr_of_mut, NonNull};
use std::sync::RwLock;

/// The primary type of this crate.
/// Internally, it is a pointer to a leaked [`InternedData`] struct, which itself
/// is a representation of a string.
/// Because it is a pointer, it is `Copy`, and equality checking is a single instruction.
#[derive(Copy, Clone, Debug)]
pub struct FastStr(NonNull<*const u8>);

assert_eq_size!(FastStr, usize);
assert_eq_size!(Option<FastStr>, usize);

unsafe impl Send for FastStr {}

unsafe impl Sync for FastStr {}

impl FastStr {
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

impl PartialEq for FastStr {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<&str> for FastStr {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl Eq for FastStr {}

impl Into<String> for FastStr {
    fn into(self) -> String {
        self.as_str().to_owned()
    }
}

impl AsRef<str> for FastStr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::hash::Hash for FastStr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}
#[derive(Debug)]
struct InternTable(RwLock<HashSet<FastStr>>);

impl InternTable {
    pub const fn new() -> Self {
        // hasher doesn't actually matter because we never use it.
        // we only manually caculate the hash

        // this looks insane but BuildHasherDefault is size 0, so it's actually
        // totally fine. however, we have to do it because DefaultHashBuilder doesn't implement
        // default
        let hasher: BuildHasherDefault<ahash::AHasher> = unsafe {
            std::mem::transmute(())
        };
        Self(RwLock::new(HashSet::with_hasher(hasher)))
    }

    pub fn get_or_intern(&self, s: &str) -> FastStr {
        let map = self.0.read().unwrap();
        let hasher = map.hasher().clone();
        let hash = hasher.hash_one(s);
        if let Some(fast_str) = map.raw_table().get(hash, |q| q.0.as_str() == s) {
            return fast_str.0;
        }
        drop(map);
        let mut map = self.0.write().unwrap();
        // we have to get again because it's possible that another thread
        // wrote to the map while we were waiting for the lock
        let raw = map.raw_table_mut();
        if let Some(fast_str) = raw.get(hash, |q| q.0.as_str() == s) {
            return fast_str.0;
        }
        let fast_str = InternedData::construct(s);
        let inserted = raw.insert_entry(hash, (fast_str, ()), |x| hasher.hash_one(x));
        inserted.0
    }
}

static GLOBAL_TABLE: InternTable = InternTable::new();

const EMPTY_FAST_STR: FastStr = FastStr(unsafe {
    // we're okay doing this because if the pointer pointed to the end
    // of memory, we'd be OOM anyway.
    NonNull::new_unchecked(usize::MAX as *mut *const u8)
});


/// This is the data that gets interned by the library.
#[repr(C)]
struct InternedData {
    len: usize,
    data: [u8],
}

// implementation taken from https://www.reddit.com/r/rust/comments/mq3kqe/comment/gue0du1/?utm_source=reddit&utm_medium=web2x&context=3
impl InternedData {
    pub fn construct(s: &str) -> FastStr {
        let interned = InternedData::new(s);
        let leaked = Box::leak(interned);
        let ptr = leaked as *const InternedData;
        FastStr(unsafe {
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
    fn it_works() {
        let s = FastStr::new("Hello");
        assert_eq!(s, "Hello");
        // let s = Box::new(InternedData {
        //     len: 3,
        //     data: *[1, 2, 3],
        // });
    }
}
