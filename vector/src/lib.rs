#![feature(ptr_internals, allocator_api, alloc_layout_extra)]

use std::alloc::{ Alloc, GlobalAlloc, Layout, Global, handle_alloc_error };
use std::fmt;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::marker::PhantomData;
use std::ptr::{Unique, NonNull, self};

#[derive(Clone)]
struct RawVec<T> {
    ptr: Unique<T>,
    cap: usize,
}

impl<T> Drop for RawVec<T> {
    fn drop(&mut self) {
        let item_size = mem::size_of::<T>();
        if self.cap != 0 && item_size != 0 {
            unsafe {
                let c: NonNull<T> = self.ptr.into();
                Global.dealloc(c.cast(), Layout::array::<T>(self.cap).unwrap())
            }
        }
    }
}

impl<T> fmt::Debug for RawVec<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "ptr {:?} cap {}", self.ptr, self.cap)
    }
}

impl<T> RawVec<T> {

    fn new() -> Self {
        let size_of = mem::size_of::<T>();
        let cap = if size_of == 0 { !0 } else { 0 };
        RawVec { ptr: Unique::empty(), cap, }
    }
    fn grow(&mut self) {
        unsafe {
            let align = mem::align_of::<T>();
            let item_size = mem::size_of::<T>();
            println!("align: {} size: {} cap: {} ptr: {:?}", align, item_size, self.cap, self.ptr);

            let (new_cap, ptr) = if self.cap == 0 {
                let ptr = Global.alloc(Layout::array::<T>(1).unwrap());
                (1, ptr)
            } else {
                let new_cap = self.cap * 2;
                let c: NonNull<T> = self.ptr.into();
                let ptr = Global.realloc(
                    c.cast(),
                    Layout::array::<T>(self.cap).unwrap(),
                    Layout::array::<T>(new_cap).unwrap().size()
                );

                (new_cap, ptr)
            };

            if ptr.is_err() {
                handle_alloc_error(Layout::from_size_align_unchecked(
                    new_cap * item_size,
                    align
                ))
            }

            let ptr = ptr.unwrap();
            self.ptr = Unique::new_unchecked(ptr.as_ptr() as *mut _);
            self.cap = new_cap;
        }
        println!("new cap: {}", self.cap)
    }
}

#[derive(Clone)]
pub struct Vector<T> {
    buff: RawVec<T>,
    len: usize,
}

impl<T> Vector<T> {
    pub fn new() -> Self {
        assert!(mem::size_of::<T>() != 0, "we ain't ready fo dat");
        Self { buff: RawVec::new(), len: 0, }
    }

    fn cap(&self) -> usize { self.buff.cap }

    fn ptr(&self) -> *mut T { self.buff.ptr.as_ptr() }

    pub fn push(&mut self, item: T) {
        if self.len == self.cap() { self.buff.grow() };

        unsafe {
            ptr::write(self.ptr().offset(self.len as isize), item);
        }
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            unsafe {
                Some(ptr::read(self.ptr().offset(self.len as isize)))
            }
        }
    }

    pub fn insert(&mut self, idx: usize, item: T) {
        assert!(idx <= self.len, format!("index {} out of bounds {}", idx, self.len));
        // grow incase of at_cap
        if self.len == self.cap() { self.buff.grow() };

        unsafe {
            if idx < self.len {
                // copy mem at idx and shift down one
                ptr::copy(
                    self.ptr().offset(idx as isize),
                    self.ptr().offset((idx as isize) + 1),
                    // shift this many elem over
                    self.len - idx,
                );
            }
            // write new item to gap
            ptr::write(self.ptr().offset(idx as isize), item);
            self.len += 1;
        }
    }

    pub fn remove(&mut self, idx: usize) -> T {
        assert!(idx < self.len, format!("index {} out of bounds {}", idx, self.len));
        unsafe {
            self.len -= 1;
            let res = ptr::read(self.ptr().offset(idx as isize));
            ptr::copy(
                // start one past
                self.ptr().offset((idx as isize) + 1),
                // smash removed offsets mem
                self.ptr().offset(idx as isize),
                // left shift this many
                self.len - idx
            );

            res
        }
    }

    pub fn into_iter(self) -> IntoIter<T> 
    where
        T: fmt::Debug,
    {    
        unsafe {
            let iter = RawIter::new(&self);
            let buff = ptr::read(&self.buff);
            println!("{:?}", buff);

            mem::forget(self);

            IntoIter {
                _buff: buff,
                iter,
            }
        }
    }

    pub fn drain(&mut self) -> Drain<T> {
        unsafe {
            let iter = RawIter::new(&self);

            self.len = 0; 

            Drain {
                iter,
                _vec: PhantomData
            }
        }
    }
}
impl<T> PartialEq for Vector<T>
where
    T: PartialEq
{
    fn eq(&self, other: &Self) -> bool {
        self[..] == other[..]
    }
}
impl<T> Drop for Vector<T> {
    fn drop(&mut self) {
        while let Some(_) = self.pop() {}
    }
}
impl<T> Deref for Vector<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        unsafe {
            ::std::slice::from_raw_parts(self.ptr(), self.len)
        }
    }
}
impl<T> DerefMut for Vector<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe {
            ::std::slice::from_raw_parts_mut(self.ptr(), self.len)
        }
    }
}
impl<T> fmt::Debug for Vector<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[ ")?;
        for (idx, x) in self.iter().enumerate() {
            if (self.len() - 1) == idx {
                return write!(f, "{:?} ]", x)
            }
            write!(f, "{:?}, ", x)?;
        }
        write!(f, "]")
    }
}

impl<T> From<Vec<T>> for Vector<T> {
    fn from(mut vec: Vec<T>) -> Vector<T> {
        let ptr = Unique::new(vec.as_mut_ptr());
        let ptr = ptr.unwrap();

        let cap = vec.capacity();
        let len = vec.len();

        mem::forget(vec);

        let buff = RawVec { ptr, cap, };
        Vector { buff, len, }
    }
}

impl<T> Into<Vec<T>> for Vector<T> {
    fn into(self) -> Vec<T> {
        let ptr = self.ptr();
        let cap = self.cap();
        let len = self.len();

        mem::forget(self);

        unsafe {
            Vec::from_raw_parts(ptr, len, cap)
        }
    }
}


struct RawIter<T> {
    start: *const T,
    end: *const T,
}
impl<T> RawIter<T> {
    unsafe fn new(slice: &[T]) -> Self {
        println!("slice as ptr {:?}", slice.as_ptr());
        RawIter{
            start: slice.as_ptr(),
            end: if mem::size_of::<T>() == 0 {
                println!("size of is ZERO");
                ((slice.as_ptr() as usize) + slice.len()) as *const _
            } else if slice.len() == 0 {
                println!("len of is ZERO");
                slice.as_ptr()
            } else {
                println!("ptr: {:?} len: {}", slice.as_ptr().offset(slice.len() as isize), slice.len());
                slice.as_ptr().offset(slice.len() as isize)
            }
        }
    }
}
impl<T> Iterator for RawIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                println!("before iter {:?}", self.start);
                let res = ptr::read(self.start);
                self.start = if mem::size_of::<T>() == 0 {
                    (self.start as usize + 1) as *const _
                } else {
                    self.start.offset(1)
                };
                println!("after iter {:?}", self.start);
                Some(res)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let item_size = mem::size_of::<T>();
        let len = (self.end as usize - self.start as usize)
            / if item_size == 0 { 1 } else { item_size };
        (len, Some(len))
    }
}
impl<T> DoubleEndedIterator for RawIter<T> {
    fn next_back(&mut self) -> Option<T> {
        if self.start == self.end {
            None
        } else {
            unsafe {
                self.end = if mem::size_of::<T>() == 0 {
                    (self.end as usize - 1) as *const _
                } else {
                    self.end.offset(-1)
                };
                Some(ptr::read(self.end))
            }
        }
    }
}


pub struct IntoIter<T> {
    _buff: RawVec<T>,
    iter: RawIter<T>,
}
impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> { self.iter.next() }
    fn size_hint(&self) -> (usize, Option<usize>) { self.iter.size_hint() }
}
impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<T> { self.iter.next_back() }
}
impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) { for _ in &mut *self {} }
}


pub struct Drain<'a, T> {
    _vec: PhantomData<&'a mut Vector<T>>,
    iter: RawIter<T>,
}
impl<'a, T> Iterator for Drain<'a, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> { self.iter.next() }
    fn size_hint(&self) -> (usize, Option<usize>) { self.iter.size_hint() }
}
impl<'a, T> DoubleEndedIterator for Drain<'a, T> {
    fn next_back(&mut self) -> Option<T> { self.iter.next_back() }
}
impl<'a, T> Drop for Drain<'a, T> {
    fn drop(&mut self) { for _ in &mut *self {} }
}

macro_rules! vector {
    ($($item:expr),*) => {
        {
            let mut res = Vector::new();
            $( res.push($item); )*
            res
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_of<T>(_: &T) -> &str {
        std::any::type_name::<T>()
    }

    #[test]
    fn test_vec_insert() {
        let mut vec = Vector::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);

        let val = vec.pop();
        println!("whole enchilada {:?}", vec);
        assert_eq!(val, Some(3));
    }

    #[test]
    fn test_vec_into_iter() {
        let mut vec = Vector::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);
        vec.push(4);
        vec.push(5);

        println!("{:?}", vec);
        for (idx, val) in vec.into_iter().enumerate() {
            println!("found {:?}", val);
            assert_eq!(val, idx + 1);
        }
    }

    #[test]
    fn test_vec_macro() {
        let x = vector![10,20,30,40];

        println!("{:?}", x);
    }

    #[test]
    fn test_vec_from_into() {
        let vector = vector![10, 20, 30, 40];
        let vec_from = Vec::from(vector.deref());
        let back_vector: Vector<_> = vec_from.into();

        let vec = vec![10, 20, 30, 40];
        let to_vector = Vector::from(vec);
        assert_eq!(back_vector, to_vector);

        let back_vec: Vec<_> = to_vector.into();
        println!("{}", type_of(&vector![1, 2, 3]));
        assert_eq!(vector![1, 2, 3], vec![1, 2, 3].into())
    }
}
