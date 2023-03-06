use crate::AsyncAtomic;
use async_std::{
    future::timeout,
    task::{sleep, spawn},
    test as async_test,
};
use std::time::Duration;

const SMALL_TIMEOUT: Duration = Duration::from_millis(10);
const BIG_TIMEOUT: Duration = Duration::from_millis(1000);

#[async_test]
async fn waiting() {
    let (val, sub) = AsyncAtomic::<usize>::new(0).split();

    assert!(timeout(SMALL_TIMEOUT, sub.wait(|x| x > 0)).await.is_err());

    spawn(async move {
        sleep(SMALL_TIMEOUT).await;
        assert_eq!(val.fetch_add(1,), 0);
    });

    assert_eq!(timeout(BIG_TIMEOUT, sub.wait(|x| x > 0)).await.unwrap(), 1);
}

#[async_test]
async fn concurrent_increment() {
    const COUNT: usize = 256;
    let (val, sub) = AsyncAtomic::<usize>::new(0).split();

    for _ in 0..COUNT {
        let val = val.clone();
        spawn(async move {
            sleep(SMALL_TIMEOUT).await;
            val.fetch_add(1);
        });
    }

    assert_eq!(
        timeout(BIG_TIMEOUT, sub.wait(|x| x == COUNT))
            .await
            .unwrap(),
        COUNT
    );
}

#[async_test]
async fn ping_pong() {
    const PROD_VAL: usize = 29;
    const CONS_VAL: usize = 17;

    let (val, sub) = AsyncAtomic::<usize>::new(0).split();

    spawn({
        let val = val.clone();
        async move {
            for _ in 0..CONS_VAL {
                sleep(SMALL_TIMEOUT).await;
                val.fetch_add(PROD_VAL);
            }
        }
    });

    for _ in 0..PROD_VAL {
        sub.wait_and_update(|x| {
            if x >= CONS_VAL {
                Some(x - CONS_VAL)
            } else {
                None
            }
        })
        .await;
    }

    assert_eq!(val.load(), 0);
}
