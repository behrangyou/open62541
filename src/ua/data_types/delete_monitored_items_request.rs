use crate::ua;

crate::data_type!(DeleteMonitoredItemsRequest);

impl DeleteMonitoredItemsRequest {
    #[must_use]
    pub fn with_monitored_item_ids(mut self, monitored_item_ids: &[ua::MonitoredItemId]) -> Self {
        let array = ua::Array::from_iter(
            monitored_item_ids
                .iter()
                .map(|id| ua::UInt32::new(id.into_inner())),
        );
        array.move_into_raw(
            &mut self.0.monitoredItemIdsSize,
            &mut self.0.monitoredItemIds,
        );
        self
    }
}
