use eyre::Result;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, Order, QueryOrder, QuerySelect};

pub async fn get_latest_nonce<T, C>(db: &DatabaseConnection, nonce_column: C) -> Result<Option<i64>>
where
    T: EntityTrait,
    C: ColumnTrait + Into<<T as EntityTrait>::Column>,
{
    let latest_nonce = T::find()
        .select_only()
        .column(nonce_column)
        .order_by(nonce_column, Order::Desc)
        .limit(1)
        .into_values::<i64, C>()
        .one(db)
        .await?;

    Ok(latest_nonce)
}
