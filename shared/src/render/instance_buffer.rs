use std::marker::PhantomData;

use wgpu::{Buffer, BufferAddress, BufferDescriptor, BufferSlice, BufferUsages, Device, Queue};

pub const INITIAL_INSTANCE_CAPACITY: usize = 256;

/// A vertex buffer of per-instance data which grows to hold whatever is written to it.
pub struct InstanceBuffer<T> {
    label: &'static str,
    buffer: Buffer,
    capacity: usize,
    count: u32,
    instance_type: PhantomData<T>,
}

impl<T: bytemuck::Pod> InstanceBuffer<T> {
    pub fn new(device: &Device, label: &'static str) -> InstanceBuffer<T> {
        InstanceBuffer {
            label,
            buffer: create_buffer::<T>(device, label, INITIAL_INSTANCE_CAPACITY),
            capacity: INITIAL_INSTANCE_CAPACITY,
            count: 0,
            instance_type: PhantomData,
        }
    }

    /// Replaces the contents.
    pub fn write(&mut self, device: &Device, queue: &Queue, instances: &[T]) {
        let grown_capacity: usize = get_grown_capacity(self.capacity, instances.len());

        if grown_capacity != self.capacity {
            self.buffer = create_buffer::<T>(device, self.label, grown_capacity);
            self.capacity = grown_capacity;
        }

        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(instances));
        self.count = u32::try_from(instances.len()).unwrap_or(u32::MAX);
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    /// `None` when nothing is written; a wgpu buffer slice cannot be empty.
    pub fn written_slice(&self) -> Option<BufferSlice<'_>> {
        if self.count == 0 {
            return None;
        }

        let written_bytes: BufferAddress = BufferAddress::from(self.count) * get_instance_size::<T>();

        Some(self.buffer.slice(..written_bytes))
    }
}

/// The current capacity while it suffices, else the next power of two at least `required`.
pub fn get_grown_capacity(capacity: usize, required: usize) -> usize {
    if required <= capacity {
        return capacity;
    }

    required.next_power_of_two()
}

fn create_buffer<T>(device: &Device, label: &'static str, capacity: usize) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: capacity as BufferAddress * get_instance_size::<T>(),
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn get_instance_size<T>() -> BufferAddress {
    std::mem::size_of::<T>() as BufferAddress
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_grown_capacity_keeps_a_sufficient_capacity() {
        assert_eq!(get_grown_capacity(256, 0), 256);
        assert_eq!(get_grown_capacity(256, 256), 256);
    }

    #[test]
    fn get_grown_capacity_grows_to_the_next_power_of_two() {
        assert_eq!(get_grown_capacity(256, 257), 512);
        assert_eq!(get_grown_capacity(256, 1500), 2048);
    }
}
