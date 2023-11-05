extern "C" {
    fn extract_bits(dst: *mut u8, src: *const u8, count: usize);
}

#[repr(align(32))]
struct AlignedArray32<const T: usize>([u8; T]);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let src = Box::new(AlignedArray32([15, 0, 255, 240, 240, 15, 0, 255]));
    let mut dst = Box::new(AlignedArray32([0; 64]));

    unsafe {
        let src = src.0.as_ptr().add(0);
        let dst = dst.0.as_mut_ptr().add(0);

        extract_bits(dst, src, 8);
    }

    println!("{:?}", &dst.0[0..32]);
    println!("{:?}", &dst.0[32..64]);

    Ok(())
}
