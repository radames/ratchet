use crate::gpu::{
    BindGroupDescriptor, BindGroupLayoutHandle, ComputePipelineHandle, GpuBindGroup, WgpuDevice,
    WorkgroupCount,
};
use crate::{drvec, rvec, OperationError, RVec, Tensor};
use derive_new::new;
use wgpu::DynamicOffset;

//Compiled op represents a single kernel invocation
//TODO: We need to be more general here, enum with encoder.copy_buffer_to_buffer as a COPY, and
//compiledOp as compute
#[derive(Debug, new)]
pub struct CompiledOp {
    pipeline_handle: ComputePipelineHandle,
    workgroup_count: WorkgroupCount,
    storage_groups: RVec<GpuBindGroup>,
    offset: DynamicOffset, //offset into the metadata uniform buffer
    kernel_name: String,
}

impl CompiledOp {
    const MAX_BINDINGS_PER_GROUP: usize = 4;

    pub fn create_storage_bind_groups(
        srcs: &[&Tensor],
        dst: &Tensor,
        bind_group_layouts: RVec<BindGroupLayoutHandle>,
        device: &WgpuDevice,
        inplace: bool,
        kernel_name: &str,
    ) -> Result<RVec<GpuBindGroup>, OperationError> {
        log::debug!("Creating storage bind groups for kernel: {}", kernel_name);
        let mut bind_group_entries = drvec![];

        for tensor in srcs.iter() {
            bind_group_entries.append(&mut tensor.bindings());
        }

        if !inplace {
            bind_group_entries.append(&mut dst.bindings());
        }

        let mut storage_groups = rvec![];
        for (group_index, bind_group_layout) in bind_group_layouts.iter().enumerate() {
            let group_range = Self::group_range(group_index, bind_group_entries.len());
            let entries = bind_group_entries[group_range].into();
            let layout = *bind_group_layout;

            let bg = device.get_or_create_bind_group(&BindGroupDescriptor { entries, layout })?;
            storage_groups.push(bg);
        }
        Ok(storage_groups)
    }

    /// Determines which bindings belong to which bind group
    fn group_range(group_index: usize, binding_counter: usize) -> std::ops::Range<usize> {
        let group_end = usize::min(
            (group_index + 1) * Self::MAX_BINDINGS_PER_GROUP,
            binding_counter,
        );
        group_index * Self::MAX_BINDINGS_PER_GROUP..group_end
    }

    pub fn workgroup_count(&self) -> &WorkgroupCount {
        &self.workgroup_count
    }

    pub fn offset(&self) -> DynamicOffset {
        self.offset
    }

    pub fn storage_groups(&self) -> &RVec<GpuBindGroup> {
        &self.storage_groups
    }

    pub fn pipeline_handle(&self) -> ComputePipelineHandle {
        self.pipeline_handle
    }

    pub fn kernel_name(&self) -> &str {
        &self.kernel_name
    }
}
