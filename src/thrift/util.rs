use std::vec::Vec;

pub fn unbox_vec<T: Clone>(vec: &Vec<Box<T>>) -> Vec<T> {
    vec.iter().map(|x| *x.clone()).collect()
}

pub fn box_vec<T: Clone>(vec: &Vec<T>) -> Vec<Box<T>> {
    vec.iter().map(|x| Box::new(x.clone())).collect()
}
