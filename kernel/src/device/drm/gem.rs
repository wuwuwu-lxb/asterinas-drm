// SPDX-License-Identifier: MPL-2.0

//! GEM (Graphics Execution Manager) 对象管理层。
//!
//! GEM 提供了一套标准的对象管理接口，Dumb Buffer 是 GEM 对象的一种类型。
//! 未来可以扩展支持其他类型的 GEM 对象（如 PRIME buffers、mts 等等）。

// 目前 Dumb Buffer 直接在 dumb.rs 中管理，
// 这里预留扩展接口的空间。

// 将来可以添加：
// - GEMObject trait
// - PRIME buffer support
// - Buffer sharing between devices
