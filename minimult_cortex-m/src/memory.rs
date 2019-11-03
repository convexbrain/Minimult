use core::mem::{MaybeUninit, size_of, align_of};
use core::marker::PhantomData;

use crate::{bk_assert};
use crate::bkptpanic::BKUnwrap;

//

fn align_up<A>(x: usize) -> usize
{
    let align = align_of::<A>();
    let y = (x + align - 1) / align;
    let y = y * align;
    y
}

//

/// Memory block used by `Minimult`
pub struct MTMemBlk<B>(MaybeUninit<B>);

impl<B> MTMemBlk<B>
{
    pub(crate) const fn new() -> MTMemBlk<B>
    {
        MTMemBlk(MaybeUninit::<B>::uninit())
    }

    fn size(&self) -> usize
    {
        size_of::<B>()
    }

    fn head(&mut self) -> usize
    {
        self.0.as_mut_ptr() as usize
    }
}

//

pub(crate) struct MTRawArray<V>
{
    head: *mut V,
    len: usize
}

impl<V> MTRawArray<V>
{
    pub(crate) fn refer<I>(&self, i: I) -> &mut V
    where I: Into<usize>
    {
        let i = i.into();
        bk_assert!(i < self.len);

        let ptr = self.head;
        let ptr = unsafe { ptr.add(i) };

        unsafe { ptr.as_mut().bk_unwrap() }
    }

    pub(crate) fn read_volatile<I>(&self, i: I) -> V
    where I: Into<usize>
    {
        let i = i.into();
        bk_assert!(i < self.len);

        let ptr = self.head;
        let ptr = unsafe { ptr.add(i) };

        unsafe { ptr.read_volatile() }
    }

    pub(crate) fn write<I>(&self, i: I, v: V)
    where I: Into<usize>
    {
        let i = i.into();
        bk_assert!(i < self.len);

        let ptr = self.head;
        let ptr = unsafe { ptr.add(i) };

        unsafe { ptr.write(v); }
    }

    pub(crate) fn write_volatile<I>(&self, i: I, v: V)
    where I: Into<usize>
    {
        let i = i.into();
        bk_assert!(i < self.len);

        let ptr = self.head;
        let ptr = unsafe { ptr.add(i) };

        unsafe { ptr.write_volatile(v); }
    }

    pub(crate) fn head(&self) -> *mut V
    {
        self.head
    }

    pub(crate) fn len(&self) -> usize
    {
        self.len
    }

    pub(crate) fn tail(&self) -> *mut V
    {
        let ptr = self.head;
        let ptr = unsafe { ptr.add(self.len) };
        ptr
    }
}

//

pub(crate) struct MTAlloc<'a>
{
    cur_pos: usize,
    end_cap: usize,
    phantom: PhantomData<&'a ()>
}

impl<'a> MTAlloc<'a>
{
    pub(crate) fn new<'b, B>(mem: &'b mut MTMemBlk<B>) -> MTAlloc<'b>
    {
        MTAlloc {
            cur_pos: mem.head(),
            end_cap: mem.head() + mem.size(),
            phantom: PhantomData
        }
    }

    pub(crate) fn array<V, A>(&mut self, len: A) -> MTRawArray<V>
    where A: Into<usize>
    {
        let len = len.into();
        let size = size_of::<V>() * len;

        let p = align_up::<V>(self.cur_pos);
        let e = p + size;

        assert!(e <= self.end_cap,
                "{} bytes shortage of memory block", size);

        self.cur_pos = e;

        MTRawArray {
            head: p as *mut V,
            len
        }
    }
}
