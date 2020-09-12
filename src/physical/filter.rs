use crate::physical::physical::*;
use arrow::array::BooleanArray;
use arrow::compute::kernels::filter;
use arrow::datatypes::Schema;
use arrow::record_batch::RecordBatch;
use std::sync::Arc;

pub struct Filter {
    field: Identifier,
    source: Arc<dyn Node>,
}

impl Filter {
    pub fn new(field: Identifier, source: Arc<dyn Node>) -> Filter {
        Filter { field, source }
    }
}

impl Node for Filter {
    fn schema(&self) -> Result<Arc<Schema>, Error> {
        self.source.schema()
    }

    fn run(
        &self,
        ctx: &ExecutionContext,
        produce: ProduceFn,
        meta_send: MetaSendFn,
    ) -> Result<(), Error> {
        let source_schema = self.source.schema()?;
        let index_of_field = source_schema.index_of(self.field.to_string().as_str())?;

        self.source.run(
            ctx,
            &mut |ctx, batch| {
                let predicate_column = batch
                    .column(index_of_field)
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .unwrap();
                let new_columns = batch
                    .columns()
                    .into_iter()
                    .map(|array_ref| filter::filter(array_ref.as_ref(), predicate_column).unwrap())
                    .collect();
                let new_batch = RecordBatch::try_new(source_schema.clone(), new_columns).unwrap();
                produce(ctx, new_batch);
                Ok(())
            },
            &mut noop_meta_send,
        );
        Ok(())
    }
}