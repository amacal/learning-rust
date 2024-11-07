use super::graph::*;
use super::heap::*;
use super::list::*;

pub struct NFA {
    counter: u16,
    transitions: Graph<4096, GuardDisabled>,
    epsilons: Collection<4096, GuardDisabled>,
}

impl NFA {
    pub fn new() -> Self {
        Self {
            counter: 0,
            transitions: Graph::new(),
            epsilons: Collection::new(),
        }
    }

    pub fn next(&mut self) -> u16 {
        self.counter = self.counter.wrapping_add(1);
        self.counter.wrapping_sub(1)
    }

    pub fn transition_count(&self) -> u16 {
        self.transitions.graph_count()
    }

    pub fn transition_at(&self, idx: u16) -> (u16, (u8, u8), u16, u16) {
        self.transitions.graph_at(idx)
    }

    pub fn transition_find(&self, src: u16, via: u8) -> Option<(u16, u16)> {
        self.transitions.graph_find(src, via)
    }

    pub fn transition_find_all(&self, src: u16) -> Option<(u16, u16)> {
        self.transitions.graph_find_all(src)
    }

    pub fn transition_inc(&mut self) -> u16 {
        self.transitions.graph_inc()
    }

    pub fn transition_add(&mut self, src: u16, via: (u8, u8), dst: u16, metadata: u16) {
        self.transitions.graph_add(src, via, dst, metadata);
    }

    pub fn transition_set(&mut self, idx: u16, src: u16, via: (u8, u8), dst: u16, metadata: u16) {
        self.transitions.graph_set(idx, src, via, dst, metadata);
    }

    fn transition_sort(&mut self) {
        self.transitions.graph_sort();
    }

    pub fn epsilon_new(&mut self) -> u16 {
        self.epsilons.list_push_head()
    }

    pub fn epsilon_count(&self) -> u16 {
        self.epsilons.list_count()
    }

    pub fn epsilon_items_resize(&mut self, idx: u16, size: u16) {
        self.epsilons.list_items_resize(idx, size)
    }

    pub fn epsilon_items_add(&mut self, idx: u16, item: u16) {
        self.epsilons.list_items_add(idx, item)
    }

    pub fn epsilon_items_set(&mut self, idx: u16, off: u16, item: u16) {
        self.epsilons.list_items_set(idx, off, item)
    }

    pub fn epsilon_items_get(&self, idx: u16, off: u16) -> u16 {
        self.epsilons.list_items_get(idx, off)
    }

    fn epsilon_items_sort(&mut self, idx: u16) {
        self.epsilons.list_items_sort(idx)
    }

    fn epsilon_items_distinct(&mut self, idx: u16) {
        self.epsilons.list_items_distinct(idx)
    }

    pub fn epsilon_items_count(&self, idx: u16) -> u16 {
        self.epsilons.list_items_count(idx)
    }

    pub fn print(&self) {
        for idx in 0..self.transition_count() {
            let transition = self.transition_at(idx);
            print!(
                "{:04x} | {:02x} - {:02x} | {:04x} | ",
                transition.0, transition.1 .0, transition.1 .1, transition.3
            );

            if transition.1 .0 > 0 {
                println!("{:04x}", transition.2);
            } else {
                for off in 0..self.epsilon_items_count(transition.2) {
                    print!("{:04x} ", self.epsilon_items_get(transition.2, off))
                }

                println!();
            }
        }

        println!();
    }
}

struct Builder {

}

#[cfg(test)]
mod tests {
    use super::*;
}
