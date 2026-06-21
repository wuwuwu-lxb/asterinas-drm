// SPDX-License-Identifier: MPL-2.0

mod driver;
mod ioctl_defs;
mod keyboard;

use alloc::collections::BTreeSet;
use core::num::NonZeroU8;

use aster_console::mode::ConsoleMode;
pub(super) use driver::{tty1_device, VtDriver};
use ostd::sync::{LocalIrqDisabled, SpinLock};
use spin::Once;

use crate::prelude::*;

const MAX_CONSOLES: usize = 63;

/// A reason why the active VT console must stay in graphics mode.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ConsoleGraphicsOwner {
    /// Userspace explicitly requested `KD_GRAPHICS`.
    UserVt,
    /// A DRM primary-node file owns the display through KMS.
    DrmFile(u64),
    /// A DRM-backed fbdev file is actively used for graphical output.
    Fbdev(u64),
}

static CONSOLE_GRAPHICS_OWNERS: Once<SpinLock<BTreeSet<ConsoleGraphicsOwner>, LocalIrqDisabled>> =
    Once::new();

fn console_graphics_owners() -> &'static SpinLock<BTreeSet<ConsoleGraphicsOwner>, LocalIrqDisabled>
{
    CONSOLE_GRAPHICS_OWNERS.call_once(|| SpinLock::new(BTreeSet::new()))
}

/// Keeps the active VT in graphics mode while at least one graphics owner exists.
pub(crate) fn enter_graphics_mode(owner: ConsoleGraphicsOwner) -> Result<()> {
    let mut owners = console_graphics_owners().lock();
    let was_empty = owners.is_empty();
    let was_inserted = owners.insert(owner);
    if was_empty && was_inserted {
        if let Err(error) = driver::set_active_console_mode(ConsoleMode::Graphics) {
            owners.remove(&owner);
            return Err(error);
        }
    }

    Ok(())
}

/// Releases one graphics owner and restores text mode when the last owner leaves.
pub(crate) fn leave_graphics_mode(owner: ConsoleGraphicsOwner) -> Result<()> {
    let mut owners = console_graphics_owners().lock();
    if !owners.remove(&owner) {
        return Ok(());
    }

    if owners.is_empty() {
        if let Err(error) = driver::set_active_console_mode(ConsoleMode::Text) {
            owners.insert(owner);
            return Err(error);
        }
    }

    Ok(())
}

/// A virtual terminal index that is always in the range `1..=MAX_CONSOLES`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct VtIndex(NonZeroU8);

impl VtIndex {
    /// Creates a `VtIndex` from a 1-based VT number.
    ///
    /// Returns `None` if `value == 0` or `value > MAX_CONSOLES`.
    const fn new(value: u8) -> Option<Self> {
        if value == 0 || value as usize > MAX_CONSOLES {
            None
        } else {
            Some(VtIndex(NonZeroU8::new(value).unwrap()))
        }
    }
}

pub(super) fn init_in_first_process() -> Result<()> {
    keyboard::init_in_first_process();
    driver::init_in_first_process()?;
    Ok(())
}
