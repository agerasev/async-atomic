extern crate std;

use crate::{prelude::*, Atomic};
use async_std::{
    future::timeout,
    task::{sleep, spawn},
    test as async_test,
};
use core::sync::atomic::AtomicUsize;
use futures::stream::StreamExt;
use std::{sync::Arc, time::Duration, vec::Vec};

const SMALL_TIMEOUT: Duration = Duration::from_millis(10);
const BIG_TIMEOUT: Duration = Duration::from_millis(1000);

#[async_test]
async fn waiting() {
    let sub = Arc::new(Atomic::<usize>::new(0));
    let val = sub.clone();

    assert!(timeout(SMALL_TIMEOUT, sub.wait(|x| x > 0)).await.is_err());

    spawn(async move {
        sleep(SMALL_TIMEOUT).await;
        assert_eq!(val.fetch_add(1), 0);
    });

    let mut v = None;
    timeout(
        BIG_TIMEOUT,
        sub.wait(|x| {
            if x > 0 {
                v = Some(x);
                true
            } else {
                false
            }
        }),
    )
    .await
    .unwrap();
    assert_eq!(v, Some(1));
}

#[async_test]
async fn concurrent_increment() {
    const COUNT: usize = 256;
    let sub = Arc::new(Atomic::<usize>::new(0));
    let val = sub.clone();

    for _ in 0..COUNT {
        let val = val.clone();
        spawn(async move {
            sleep(SMALL_TIMEOUT).await;
            val.fetch_add(1);
        });
    }

    timeout(BIG_TIMEOUT, sub.wait(|x| x == COUNT))
        .await
        .unwrap();
}

#[async_test]
async fn ping_pong() {
    const PROD_VAL: usize = 29;
    const CONS_VAL: usize = 17;

    let sub = Arc::new(Atomic::<usize>::new(0));
    let val = sub.clone();

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

#[async_test]
async fn static_() {
    static ATOMIC: Atomic<usize> = Atomic::from_impl(AtomicUsize::new(0));

    let sub = &ATOMIC;

    assert!(timeout(SMALL_TIMEOUT, sub.wait(|x| x > 0)).await.is_err());

    spawn(async move {
        sleep(SMALL_TIMEOUT).await;
        assert_eq!(ATOMIC.fetch_add(1), 0);
    });

    let mut v = None;
    timeout(
        BIG_TIMEOUT,
        sub.wait(|x| {
            if x > 0 {
                v = Some(x);
                true
            } else {
                false
            }
        }),
    )
    .await
    .unwrap();
    assert_eq!(v, Some(1));
}

#[async_test]
async fn stream() {
    const COUNT: usize = 64;
    let sub = Arc::new(Atomic::<usize>::new(0));
    let val = sub.clone();

    spawn(async move {
        for _ in 0..COUNT {
            sleep(SMALL_TIMEOUT).await;
            val.fetch_add(1);
        }
    });

    spawn(async move {
        let stream = sub.changed();
        let data = timeout(BIG_TIMEOUT, stream.take(COUNT + 1).collect::<Vec<_>>())
            .await
            .unwrap();
        assert!(data.into_iter().eq(0..=COUNT));
    })
    .await
}
