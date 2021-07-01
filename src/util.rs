pub unsafe fn convert_vec<T, U>(mut src: Vec<T>) -> Vec<U> {
    let ratio = std::mem::size_of::<T>() / std::mem::size_of::<U>();

    let length = src.len() * ratio;
    let capacity = src.capacity() * ratio;
    let ptr = src.as_mut_ptr() as *mut U;

    std::mem::forget(src);

    Vec::from_raw_parts(ptr, length, capacity)
}
