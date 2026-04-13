// SPDX-License-Identifier: MPL-2.0

//! Dumb Buffer 管理实现。

use alloc::{sync::Arc, vec::Vec};

use ostd::mm::{VmIo, PAGE_SIZE};
use spin::Once;

use crate::{
    prelude::*,
    vm::vmo::{CommitFlags, Vmo, VmoFlags, VmoOptions},
};

/// 全局 DumbBufferManager 实例
static DUMB_BUFFER_MANAGER: Once<DumbBufferManager> = Once::new();

/// Dumb Buffer 槽位大小（用于 VMO 偏移计算）
/// 16MB 的槽位大小允许最大 4096x4096x32bpp 的 buffer
const DUMB_BUFFER_SLOT_SIZE: usize = 16 * 1024 * 1024;

/// 每个 Dumb Buffer 占用的页面数
const DUMB_BUFFER_SLOT_PAGES: usize = DUMB_BUFFER_SLOT_SIZE / PAGE_SIZE;

/// Dumb Buffer 槽位对齐
const DUMB_BUFFER_ALIGN: usize = PAGE_SIZE;

/// Dumb Buffer 结构体
///
/// 表示一个简单的线性像素缓冲区。
#[derive(Debug)]
pub struct DumbBuffer {
    /// 全局唯一 ID
    id: u32,
    /// 宽度（像素）
    width: u32,
    /// 高度（像素）
    height: u32,
    /// 每像素位数
    bpp: u32,
    /// 每行字节数
    pitch: u32,
    /// 总大小（字节）
    size: u64,
    /// 数据向量（原始字节）
    data: Vec<u8>,
}

impl DumbBuffer {
    /// 创建一个新的 Dumb Buffer
    pub fn new(width: u32, height: u32, bpp: u32) -> Self {
        let pitch = (width as usize * bpp as usize / 8).next_multiple_of(4);
        let size = pitch as u64 * height as u64;
        let data = vec![0u8; size as usize];

        DumbBuffer {
            id: 0, // ID 由 manager 分配
            width,
            height,
            bpp,
            pitch: pitch as u32,
            size,
            data,
        }
    }

    /// 返回 buffer ID
    pub fn id(&self) -> u32 {
        self.id
    }

    /// 返回宽度
    pub fn width(&self) -> u32 {
        self.width
    }

    /// 返回高度
    pub fn height(&self) -> u32 {
        self.height
    }

    /// 返回每像素位数
    pub fn bpp(&self) -> u32 {
        self.bpp
    }

    /// 返回 pitch（每行字节数）
    pub fn pitch(&self) -> u32 {
        self.pitch
    }

    /// 返回总大小
    pub fn size(&self) -> u64 {
        self.size
    }

    /// 返回数据引用
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// 返回可变数据引用
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

/// ID 分配器
#[derive(Debug)]
pub struct IdAllocator {
    next_id: u32,
    free_ids: Vec<u32>,
}

impl IdAllocator {
    /// 创建一个新的 ID 分配器
    pub fn new() -> Self {
        IdAllocator {
            next_id: 1, // 从 1 开始，0 表示无效
            free_ids: Vec::new(),
        }
    }

    /// 分配一个新的 ID
    pub fn allocate(&mut self) -> u32 {
        if let Some(id) = self.free_ids.pop() {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// 释放一个 ID
    pub fn release(&mut self, id: u32) {
        if id > 0 && id < self.next_id {
            self.free_ids.push(id);
        }
    }
}

/// Dumb Buffer 管理器
///
/// 管理所有 Dumb Buffer 对象。
#[derive(Debug)]
pub struct DumbBufferManager {
    /// 所有 dumb buffer
    buffers: SpinLock<Vec<DumbBuffer>>,
    /// ID 分配器
    id_alloc: SpinLock<IdAllocator>,
    /// 全局 VMO（用于 mmap）
    vmo: Arc<Vmo>,
}

impl DumbBufferManager {
    /// 创建一个新的 DumbBufferManager
    pub fn new() -> Result<Self> {
        // 创建一个足够大的 VMO 来存储所有 dumb buffer
        // 256 个槽位 * 16MB = 4GB 的地址空间
        let max_buffers = 256;
        let vmo_size = max_buffers * DUMB_BUFFER_SLOT_SIZE;
        let vmo = VmoOptions::new(vmo_size)
            .flags(VmoFlags::RESIZABLE)
            .alloc()?;

        Ok(DumbBufferManager {
            buffers: SpinLock::new(Vec::new()),
            id_alloc: SpinLock::new(IdAllocator::new()),
            vmo,
        })
    }

    /// 创建一个新的 Dumb Buffer
    /// 返回 (handle, pitch, offset, size)
    pub fn create_dumb(&self, width: u32, height: u32, bpp: u32) -> Result<(u32, u32, u64, u64)> {
        // 验证参数
        if width == 0 || height == 0 || bpp == 0 {
            return_errno_with_message!(Errno::EINVAL, "invalid dumb buffer dimensions");
        }

        let pitch = (width as usize * bpp as usize / 8).next_multiple_of(4);
        let size = pitch as u64 * height as u64;

        // 检查大小是否超过槽位
        if size as usize > DUMB_BUFFER_SLOT_SIZE {
            return_errno_with_message!(
                Errno::EINVAL,
                "dumb buffer size exceeds slot size"
            );
        }

        let id = {
            let mut id_alloc = self.id_alloc.lock();
            id_alloc.allocate()
        };

        // 计算在 VMO 中的偏移
        let vmo_offset = (id as usize - 1) * DUMB_BUFFER_SLOT_SIZE;

        let mut buffer = DumbBuffer::new(width, height, bpp);
        buffer.id = id;

        let mut buffers = self.buffers.lock();
        buffers.push(buffer);

        // 返回 handle（等同于 id）、pitch 和 offset
        Ok((id, pitch as u32, vmo_offset as u64, size))
    }

    /// 获取 handle 对应的 buffer 索引
    fn find_buffer_index(&self, handle: u32) -> Option<usize> {
        let buffers = self.buffers.lock();
        buffers.iter().position(|b| b.id == handle)
    }

    /// 获取 handle 对应的 VMO 偏移
    pub fn get_offset(&self, handle: u32) -> Result<usize> {
        // 验证 handle 存在
        if self.find_buffer_index(handle).is_none() {
            return_errno_with_message!(Errno::ENOENT, "buffer not found");
        }
        // 使用 handle 计算槽位偏移（handle 从 1 开始）
        Ok((handle as usize - 1) * DUMB_BUFFER_SLOT_SIZE)
    }

    /// 销毁一个 Dumb Buffer
    pub fn destroy_dumb(&self, handle: u32) -> Result<()> {
        let mut buffers = self.buffers.lock();
        let mut id_alloc = self.id_alloc.lock();

        let index = buffers.iter().position(|b| b.id == handle)
            .ok_or_else(|| Error::from(Errno::ENOENT))?;

        buffers.remove(index);
        id_alloc.release(handle);

        Ok(())
    }

    /// 获取 VMO 用于 mmap
    pub fn vmo(&self) -> Arc<Vmo> {
        self.vmo.clone()
    }

    /// 写入数据到 buffer
    pub fn write_data(&self, handle: u32, offset: usize, data: &[u8]) -> Result<()> {
        let mut buffers = self.buffers.lock();
        let buffer = buffers.iter_mut()
            .find(|b| b.id == handle)
            .ok_or_else(|| Error::from(Errno::ENOENT))?;

        let buf_offset = offset;
        if buf_offset >= buffer.data.len() {
            return_errno_with_message!(Errno::EINVAL, "offset out of bounds");
        }

        let end = (buf_offset + data.len()).min(buffer.data.len());
        buffer.data[buf_offset..end].copy_from_slice(&data[..end - buf_offset]);

        // 同步到 VMO
        let vmo_offset = (buffer.id as usize - 1) * DUMB_BUFFER_SLOT_SIZE + buf_offset;
        self.vmo.write_bytes(vmo_offset, &data[..end - buf_offset])?;

        Ok(())
    }
}

/// 初始化全局 DumbBufferManager
pub fn init() -> Result<()> {
    let manager = DumbBufferManager::new()?;
    DUMB_BUFFER_MANAGER.call_once(|| manager);
    Ok(())
}

/// 获取全局 DumbBufferManager
pub fn get_manager() -> &'static DumbBufferManager {
    DUMB_BUFFER_MANAGER.get()
        .expect("DumbBufferManager not initialized")
}
