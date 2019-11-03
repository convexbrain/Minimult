use num_integer::Integer;

use crate::memory::MTRawArray;
use crate::bk_assert;
use crate::bkptpanic::BKUnwrap;

//

pub(crate) struct MTBHeapDList<I, K>
{
    array: MTRawArray<Option<(I, K)>>,
    n_bheap: I,
    n_flist: I
}

impl<I, K> MTBHeapDList<I, K>
where I: Integer + Into<usize> + Copy, K: Ord
{
    pub(crate) fn new(array: MTRawArray<Option<(I, K)>>) -> MTBHeapDList<I, K>
    {
        MTBHeapDList {
            array,
            n_bheap: I::zero(),
            n_flist: I::zero()
        }
    }

    fn replace(&mut self, pos0: I, pos1: I)
    {
        if pos0 != pos1 {
            let tmp0 = self.array.refer(pos0).take();
            let tmp1 = self.array.refer(pos1).take();
            self.array.write(pos0, tmp1);
            self.array.write(pos1, tmp0);
        }
    }

    fn up_bheap(&mut self)
    {
        let two = I::one() + I::one();

        if self.n_bheap > I::zero() {
            let mut pos = self.n_bheap - I::one();

            while pos > I::zero() {
                let parent = (pos - I::one()) / two;

                let key_pos = &self.array.refer(pos).as_ref().bk_unwrap().1;
                let key_parent = &self.array.refer(parent).as_ref().bk_unwrap().1;

                if key_pos >= key_parent {
                    break;
                }

                self.replace(pos, parent);
                pos = parent;
            }
        }
    }

    fn down_bheap(&mut self)
    {
        let two = I::one() + I::one();

        let mut pos = I::zero();

        while pos < self.n_bheap / two {
            let child0 = (pos * two) + I::one();
            let child1 = (pos * two) + two;

            let key_pos = &self.array.refer(pos).as_ref().bk_unwrap().1;
            let key_child0 = &self.array.refer(child0).as_ref().bk_unwrap().1;

            let (child, key_child) = if child1 < self.n_bheap {
                let key_child1 = &self.array.refer(child1).as_ref().bk_unwrap().1;

                if key_child0 <= key_child1 {
                    (child0, key_child0)
                }
                else {
                    (child1, key_child1)
                }
            }
            else {
                (child0, key_child0)
            };

            if key_pos < key_child {
                break;
            }

            self.replace(pos, child);
            pos = child;
        }
    }

    fn flist_to_bheap(&mut self, pos: I)
    {
        bk_assert!(pos >= self.n_bheap);
        bk_assert!(pos < self.n_bheap + self.n_flist);

        // replace flist pos <=> flist head
        self.replace(pos, self.n_bheap);

        // flist head <=> bheap tail
        self.n_flist = self.n_flist - I::one();
        self.n_bheap = self.n_bheap + I::one();

        self.up_bheap();
    }

    pub(crate) fn add_bheap(&mut self, id: I, key: K)
    {
        // add flist tail
        let pos = self.n_bheap + self.n_flist;
        self.array.write(pos, Some((id, key)));
        self.n_flist = self.n_flist + I::one();

        // flist tail => bheap
        self.flist_to_bheap(pos);
    }

    pub(crate) fn bheap_h_to_flist_h(&mut self)
    {
        bk_assert!(self.n_bheap > I::zero());
        
        // replace bheap head <=> bheap tail
        let pos1 = self.n_bheap - I::one();
        self.replace(I::zero(), pos1);

        // bheap tail <=> flist head
        self.n_flist = self.n_flist + I::one();
        self.n_bheap = self.n_bheap - I::one();

        self.down_bheap();
    }

    pub(crate) fn round_bheap_h(&mut self)
    {
        self.bheap_h_to_flist_h();

        self.flist_to_bheap(self.n_bheap);
    }

    pub(crate) fn remove_bheap_h(&mut self)
    {
        self.bheap_h_to_flist_h();

        // replace flist head <=> flist tail
        let pos1 = self.n_bheap + self.n_flist - I::one();
        self.replace(self.n_bheap, pos1);

        // remove flist tail
        self.array.write(pos1, None);
        self.n_flist = self.n_flist - I::one();
    }

    pub(crate) fn bheap_h(&self) -> Option<I>
    {
        if self.n_bheap > I::zero() {
            Some(self.array.refer(I::zero()).as_ref().bk_unwrap().0)
        }
        else {
            None
        }
    }

    pub(crate) fn flist_scan<F>(&mut self, to_bheap: F)
    where F: Fn(I) -> bool
    {
        let pos_b = self.n_bheap;
        let pos_e = pos_b + self.n_flist;

        let mut pos = pos_b;
        while pos < pos_e {
            if to_bheap(self.array.refer(pos).as_ref().bk_unwrap().0) {
                self.flist_to_bheap(pos);
            }
            pos = pos + I::one();
        }
    }
}
