// Copyright © 2024 Institute of Software, CAS. All rights reserved.
//
// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs::File;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};

use crate::ioctls::Result;
use crate::kvm_ioctls::{KVM_GET_DEVICE_ATTR, KVM_HAS_DEVICE_ATTR, KVM_SET_DEVICE_ATTR};
use kvm_bindings::kvm_device_attr;
use vmm_sys_util::errno;
use vmm_sys_util::ioctl::{ioctl_with_mut_ref, ioctl_with_ref};

/// Wrapper over the file descriptor obtained when creating an emulated device in the kernel.
#[derive(Debug)]
pub struct DeviceFd {
    fd: File,
}

impl DeviceFd {
    /// Tests whether a device supports a particular attribute.
    ///
    /// See the documentation for `KVM_HAS_DEVICE_ATTR`.
    /// # Arguments
    ///
    /// * `device_attr` - The device attribute to be tested. `addr` field is ignored.
    pub fn has_device_attr(&self, device_attr: &kvm_device_attr) -> Result<()> {
        // SAFETY: We are calling this function with a Device fd, and we trust the kernel.
        let ret = unsafe { ioctl_with_ref(self, KVM_HAS_DEVICE_ATTR(), device_attr) };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Sets a specified piece of device configuration and/or state.
    ///
    /// See the documentation for `KVM_SET_DEVICE_ATTR`.
    /// # Arguments
    ///
    /// * `device_attr` - The device attribute to be set.
    ///
    /// # Example
    ///
    /// Configuring a VFIO device using `set_device_attr`. Note that VFIO
    /// devices are not yet available on RISC-V The patch for QEMU:
    /// https://lore.kernel.org/all/20240903201633.93182-1-dbarboza@ventanamicro.com/
    /// and patch for linux kernel
    /// https://github.com/ventanamicro/linux/tree/dev-upstream are both not
    /// upstreamed. Disabling VFIO device test for RISC-V at the time being.
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    ///
    /// # #[cfg(not(target_arch = "riscv64"))]
    /// # {
    /// # use kvm_bindings::{
    /// #     kvm_device_type_KVM_DEV_TYPE_VFIO,
    /// #     KVM_DEV_VFIO_GROUP, KVM_DEV_VFIO_GROUP_ADD, KVM_CREATE_DEVICE_TEST
    /// # };
    /// let mut device = kvm_bindings::kvm_create_device {
    ///     type_: kvm_device_type_KVM_DEV_TYPE_VFIO,
    ///     fd: 0,
    ///     flags: KVM_CREATE_DEVICE_TEST,
    /// };
    ///
    /// let device_fd = vm
    ///     .create_device(&mut device)
    ///     .expect("Cannot create KVM device");
    ///
    /// let dist_attr = kvm_bindings::kvm_device_attr {
    ///     group: KVM_DEV_VFIO_GROUP,
    ///     attr: u64::from(KVM_DEV_VFIO_GROUP_ADD),
    ///     addr: 0x0,
    ///     flags: 0,
    /// };
    ///
    /// if (device_fd.has_device_attr(&dist_attr).is_ok()) {
    ///     device_fd.set_device_attr(&dist_attr).unwrap();
    /// }
    /// # }
    /// ```
    pub fn set_device_attr(&self, device_attr: &kvm_device_attr) -> Result<()> {
        // SAFETY: We are calling this function with a Device fd, and we trust the kernel.
        let ret = unsafe { ioctl_with_ref(self, KVM_SET_DEVICE_ATTR(), device_attr) };
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }

    /// Gets a specified piece of device configuration and/or state.
    ///
    /// See the documentation for `KVM_GET_DEVICE_ATTR`.
    ///
    /// # Arguments
    ///
    /// * `device_attr` - The device attribute to be get.
    ///                   Note: This argument serves as both input and output.
    ///                   When calling this function, the user should explicitly provide
    ///                   valid values for the `group` and the `attr` field of the
    ///                   `kvm_device_attr` structure, and a valid userspace address
    ///                   (i.e. the `addr` field) to access the returned device attribute
    ///                   data.
    ///
    /// # Returns
    ///
    /// * Returns the last occured `errno` wrapped in an `Err`.
    /// * `device_attr` - The `addr` field of the `device_attr` structure will point to
    ///                   the device attribute data.
    ///
    /// # Safety
    ///
    /// The caller is responsible for the validity of the `device_attr` argument,
    /// including that it is safe to write to the `addr` member.
    ///
    /// # Examples
    ///
    /// Getting the number of IRQs for a GICv2 device on an aarch64 platform
    ///
    /// ```rust
    /// # extern crate kvm_ioctls;
    /// # extern crate kvm_bindings;
    /// # use kvm_ioctls::Kvm;
    /// let kvm = Kvm::new().unwrap();
    /// let vm = kvm.create_vm().unwrap();
    ///
    /// # #[cfg(target_arch = "aarch64")]
    /// # {
    /// use kvm_bindings::{
    ///     kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V2, kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3,
    ///     KVM_DEV_ARM_VGIC_GRP_NR_IRQS,
    /// };
    ///
    /// // Create a GIC device.
    /// let mut gic_device = kvm_bindings::kvm_create_device {
    ///     type_: kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3,
    ///     fd: 0,
    ///     flags: 0,
    /// };
    /// let device_fd = match vm.create_device(&mut gic_device) {
    ///     Ok(fd) => fd,
    ///     Err(_) => {
    ///         gic_device.type_ = kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V2;
    ///         vm.create_device(&mut gic_device)
    ///             .expect("Cannot create KVM vGIC device")
    ///     }
    /// };
    ///
    /// let mut data: u32 = 0;
    /// let mut gic_attr = kvm_bindings::kvm_device_attr::default();
    /// gic_attr.group = KVM_DEV_ARM_VGIC_GRP_NR_IRQS;
    /// gic_attr.addr = &mut data as *mut u32 as u64;
    ///
    /// // SAFETY: gic_attr.addr is safe to write to.
    /// unsafe { device_fd.get_device_attr(&mut gic_attr) }.unwrap();
    /// # }
    /// ```
    pub unsafe fn get_device_attr(&self, device_attr: &mut kvm_device_attr) -> Result<()> {
        // SAFETY: Caller has ensured device_attr.addr is safe to write to.
        // We are calling this function with a Device fd,  we trust the kernel.
        let ret = ioctl_with_mut_ref(self, KVM_GET_DEVICE_ATTR(), device_attr);
        if ret != 0 {
            return Err(errno::Error::last());
        }
        Ok(())
    }
}

/// Helper function for creating a new device.
pub fn new_device(dev_fd: File) -> DeviceFd {
    DeviceFd { fd: dev_fd }
}

impl AsRawFd for DeviceFd {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

impl FromRawFd for DeviceFd {
    /// # Safety
    ///
    /// This function is unsafe as the primitives currently returned have the contract that
    /// they are the sole owner of the file descriptor they are wrapping. Usage of this function
    /// could accidentally allow violating this contract which can cause memory unsafety in code
    /// that relies on it being true.
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        DeviceFd {
            fd: File::from_raw_fd(fd),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::undocumented_unsafe_blocks)]
    use super::*;
    use crate::ioctls::system::Kvm;
    #[cfg(target_arch = "x86_64")]
    use kvm_bindings::{
        kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3, kvm_device_type_KVM_DEV_TYPE_VFIO,
        KVM_DEV_VFIO_GROUP, KVM_DEV_VFIO_GROUP_ADD,
    };
    #[cfg(target_arch = "aarch64")]
    use kvm_bindings::{KVM_DEV_VFIO_GROUP, KVM_DEV_VFIO_GROUP_ADD};

    use kvm_bindings::KVM_CREATE_DEVICE_TEST;

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_create_device() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();

        let mut gic_device = kvm_bindings::kvm_create_device {
            type_: kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3,
            fd: 0,
            flags: KVM_CREATE_DEVICE_TEST,
        };
        // This fails on x86_64 because there is no VGIC there.
        vm.create_device(&mut gic_device).unwrap_err();

        gic_device.type_ = kvm_device_type_KVM_DEV_TYPE_VFIO;

        let device_fd = vm
            .create_device(&mut gic_device)
            .expect("Cannot create KVM device");

        // Following lines to re-construct device_fd are used to test
        // DeviceFd::from_raw_fd() and DeviceFd::as_raw_fd().
        let raw_fd = unsafe { libc::dup(device_fd.as_raw_fd()) };
        assert!(raw_fd >= 0);
        let device_fd = unsafe { DeviceFd::from_raw_fd(raw_fd) };

        let dist_attr = kvm_bindings::kvm_device_attr {
            group: KVM_DEV_VFIO_GROUP,
            attr: u64::from(KVM_DEV_VFIO_GROUP_ADD),
            addr: 0x0,
            flags: 0,
        };

        let mut dist_attr_mut = dist_attr;

        // We are just creating a test device. Creating a real device would make the CI dependent
        // on host configuration (like having /dev/vfio). We expect this to fail.
        device_fd.has_device_attr(&dist_attr).unwrap_err();
        unsafe { device_fd.get_device_attr(&mut dist_attr_mut) }.unwrap_err();
        device_fd.set_device_attr(&dist_attr).unwrap_err();
        assert_eq!(errno::Error::last().errno(), 25);
    }

    #[test]
    #[cfg(target_arch = "aarch64")]
    fn test_create_device() {
        use crate::ioctls::vm::{create_gic_device, request_gic_init, set_supported_nr_irqs};
        use kvm_bindings::{
            kvm_device_type_KVM_DEV_TYPE_FSL_MPIC_20, KVM_DEV_ARM_VGIC_GRP_NR_IRQS,
        };
        use vmm_sys_util::errno::Error;

        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();

        let mut gic_device = kvm_bindings::kvm_create_device {
            type_: kvm_device_type_KVM_DEV_TYPE_FSL_MPIC_20,
            fd: 0,
            flags: KVM_CREATE_DEVICE_TEST,
        };
        // This fails on aarch64 as it does not use MPIC (MultiProcessor Interrupt Controller),
        // it uses the VGIC.
        vm.create_device(&mut gic_device).unwrap_err();

        let device_fd = create_gic_device(&vm, 0);

        // GICv3 on arm/aarch64 requires an online vCPU prior to setting device attributes,
        // see: https://www.kernel.org/doc/html/latest/virt/kvm/devices/arm-vgic-v3.html
        vm.create_vcpu(0).unwrap();

        // Following lines to re-construct device_fd are used to test
        // DeviceFd::from_raw_fd() and DeviceFd::as_raw_fd().
        let raw_fd = unsafe { libc::dup(device_fd.as_raw_fd()) };
        assert!(raw_fd >= 0);
        let device_fd = unsafe { DeviceFd::from_raw_fd(raw_fd) };

        // Set some attribute that does not apply to VGIC, expect the test to fail.
        let dist_attr = kvm_bindings::kvm_device_attr {
            group: KVM_DEV_VFIO_GROUP,
            attr: u64::from(KVM_DEV_VFIO_GROUP_ADD),
            addr: 0x0,
            flags: 0,
        };
        device_fd.has_device_attr(&dist_attr).unwrap_err();

        // Set maximum supported number of IRQs of the vGIC device to 128.
        set_supported_nr_irqs(&device_fd, 128);

        // Initialize valid vGIC device.
        request_gic_init(&device_fd);

        // Test `get_device_attr`. Here we try to extract the maximum supported number of IRQs.
        // This value should be saved in the address provided to the ioctl.
        let mut data: u32 = 0;

        let mut gic_attr = kvm_bindings::kvm_device_attr {
            group: KVM_DEV_ARM_VGIC_GRP_NR_IRQS,
            addr: data as u64,
            ..Default::default()
        };

        // Without properly providing the address to where the
        // value will be stored, the ioctl fails with EFAULT.
        let res = unsafe { device_fd.get_device_attr(&mut gic_attr) };
        assert_eq!(res, Err(Error::new(libc::EFAULT)));

        gic_attr.addr = &mut data as *mut u32 as u64;
        unsafe { device_fd.get_device_attr(&mut gic_attr) }.unwrap();
        // The maximum supported number of IRQs should be 128, same as the value
        // when we initialize the GIC.
        assert_eq!(data, 128);
    }

    #[test]
    #[cfg(target_arch = "riscv64")]
    fn test_create_device() {
        use crate::ioctls::vm::{create_aia_device, request_aia_init, set_supported_nr_irqs};
        use kvm_bindings::{
            kvm_device_attr, kvm_device_type_KVM_DEV_TYPE_FSL_MPIC_20,
            KVM_DEV_RISCV_AIA_ADDR_APLIC, KVM_DEV_RISCV_AIA_CONFIG_SRCS,
            KVM_DEV_RISCV_AIA_GRP_ADDR, KVM_DEV_RISCV_AIA_GRP_CONFIG,
        };
        use vmm_sys_util::errno::Error;

        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();

        let mut aia_device = kvm_bindings::kvm_create_device {
            type_: kvm_device_type_KVM_DEV_TYPE_FSL_MPIC_20,
            fd: 0,
            flags: KVM_CREATE_DEVICE_TEST,
        };
        // This fails on riscv64 as it does not use MPIC (MultiProcessor Interrupt Controller),
        // it uses the vAIA.
        vm.create_device(&mut aia_device).unwrap_err();

        let device_fd = create_aia_device(&vm, 0);

        // AIA on riscv64 requires at least one online vCPU prior to setting
        // device attributes. Otherwise it would fail when trying to set address
        // of IMSIC.
        vm.create_vcpu(0).unwrap();

        // Following lines to re-construct device_fd are used to test
        // DeviceFd::from_raw_fd() and DeviceFd::as_raw_fd().
        let raw_fd = unsafe { libc::dup(device_fd.as_raw_fd()) };
        assert!(raw_fd >= 0);
        let device_fd = unsafe { DeviceFd::from_raw_fd(raw_fd) };

        // Set maximum supported number of IRQs of the vAIA device to 128.
        set_supported_nr_irqs(&device_fd, 128);

        // Before request vAIA device to initialize, APLIC and IMSIC must be set
        let aplic_addr: u64 = 0x4000;
        device_fd
            .set_device_attr(&kvm_device_attr {
                group: KVM_DEV_RISCV_AIA_GRP_ADDR,
                attr: u64::from(KVM_DEV_RISCV_AIA_ADDR_APLIC),
                addr: &aplic_addr as *const u64 as u64,
                flags: 0,
            })
            .unwrap();
        let imsic_addr: u64 = 0x8000;
        device_fd
            .set_device_attr(&kvm_device_attr {
                group: KVM_DEV_RISCV_AIA_GRP_ADDR,
                attr: 1u64,
                addr: &imsic_addr as *const u64 as u64,
                flags: 0,
            })
            .unwrap();

        // Initialize valid vAIA device.
        request_aia_init(&device_fd);

        // Test `get_device_attr`. Here we try to extract the maximum supported number of IRQs.
        // This value should be saved in the address provided to the ioctl.
        let mut data: u32 = 0;

        let mut aia_attr = kvm_bindings::kvm_device_attr {
            group: KVM_DEV_RISCV_AIA_GRP_CONFIG,
            attr: u64::from(KVM_DEV_RISCV_AIA_CONFIG_SRCS),
            addr: data as u64,
            ..Default::default()
        };

        // Without properly providing the address to where the
        // value will be stored, the ioctl fails with EFAULT.
        let res = unsafe { device_fd.get_device_attr(&mut aia_attr) };
        assert_eq!(res, Err(Error::new(libc::EFAULT)));

        aia_attr.addr = &mut data as *mut u32 as u64;
        unsafe { device_fd.get_device_attr(&mut aia_attr) }.unwrap();
        // The maximum supported number of IRQs should be 128, same as the value
        // when we initialize the AIA.
        assert_eq!(data, 128);
    }
}
