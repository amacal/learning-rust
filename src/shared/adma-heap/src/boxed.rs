use super::*;

impl<T: HeapLifetime> Boxed<T> {
    pub fn droplet(self) -> Droplet<Boxed<T>> {
        fn destroy<T: HeapLifetime>(target: &mut Boxed<T>) {
            // tracing releases boxed may help in any naive troubleshooting
            trace2(b"releasing boxed droplet; addr=%x, size=%d\n", target.root, target.len);

            target.dtor();

            // use syscall to free memory without error propagation
            sys_munmap(target.root, target.len);
        }

        // tracing conversion heap to a droplet may help in any naive troubleshooting
        trace2(b"creating boxed droplet; addr=%x, size=%d\n", self.root, self.len);

        Droplet::from(self, destroy)
    }
}
