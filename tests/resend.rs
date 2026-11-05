use bevy::{prelude::*, state::app::StatesPlugin};
use bevy_replicon::{
    prelude::*,
    server::ResendReplication,
    shared::server_entity_map::ServerEntityMap,
    test_app::{ServerTestAppExt, TestClientEntity},
};
use serde::{Deserialize, Serialize};
use test_log::test;

#[test]
fn restores_drifted_component_in_place() {
    let mut server_app = App::new();
    let mut client_app = App::new();
    for app in [&mut server_app, &mut client_app] {
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            RepliconPlugins.set(ServerPlugin::new(PostUpdate)),
        ))
        .replicate::<Counter>()
        .finish();
    }

    server_app.connect_client(&mut client_app);

    let server_entity = server_app.world_mut().spawn((Replicated, Counter(1))).id();

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();
    server_app.exchange_with_client(&mut client_app);

    let client_entity = *client_app
        .world()
        .resource::<ServerEntityMap>()
        .to_client()
        .get(&server_entity)
        .unwrap();

    // The client drifts, and the server has nothing new to send.
    client_app
        .world_mut()
        .get_mut::<Counter>(client_entity)
        .unwrap()
        .0 = 7;

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();
    server_app.exchange_with_client(&mut client_app);
    assert_eq!(
        client_app.world().get::<Counter>(client_entity),
        Some(&Counter(7))
    );

    let test_client = **client_app.world().resource::<TestClientEntity>();
    server_app.world_mut().trigger(ResendReplication {
        client: test_client,
    });

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();
    server_app.exchange_with_client(&mut client_app);

    let mut entities = client_app
        .world_mut()
        .query_filtered::<Entity, With<Remote>>();
    assert_eq!(
        entities.iter(client_app.world()).collect::<Vec<_>>(),
        [client_entity],
        "the resend must reach the mapped entity instead of spawning another"
    );
    assert_eq!(
        client_app.world().get::<Counter>(client_entity),
        Some(&Counter(1))
    );
}

#[test]
fn restores_component_removed_on_client() {
    let mut server_app = App::new();
    let mut client_app = App::new();
    for app in [&mut server_app, &mut client_app] {
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            RepliconPlugins.set(ServerPlugin::new(PostUpdate)),
        ))
        .replicate::<Counter>()
        .finish();
    }

    server_app.connect_client(&mut client_app);

    let server_entity = server_app.world_mut().spawn((Replicated, Counter(3))).id();

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();
    server_app.exchange_with_client(&mut client_app);

    let client_entity = *client_app
        .world()
        .resource::<ServerEntityMap>()
        .to_client()
        .get(&server_entity)
        .unwrap();
    client_app
        .world_mut()
        .entity_mut(client_entity)
        .remove::<Counter>();

    let test_client = **client_app.world().resource::<TestClientEntity>();
    server_app.world_mut().trigger(ResendReplication {
        client: test_client,
    });

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();

    assert_eq!(
        client_app.world().get::<Counter>(client_entity),
        Some(&Counter(3))
    );
}

#[test]
fn keeps_despawn_made_before_resend_is_sent() {
    let mut server_app = App::new();
    let mut client_app = App::new();
    for app in [&mut server_app, &mut client_app] {
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            RepliconPlugins.set(ServerPlugin::new(PostUpdate)),
        ))
        .replicate::<Counter>()
        .finish();
    }

    server_app.connect_client(&mut client_app);

    let server_entity = server_app.world_mut().spawn((Replicated, Counter(1))).id();

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();
    server_app.exchange_with_client(&mut client_app);
    assert!(
        client_app
            .world()
            .resource::<ServerEntityMap>()
            .to_client()
            .contains_key(&server_entity)
    );

    let test_client = **client_app.world().resource::<TestClientEntity>();
    server_app.world_mut().trigger(ResendReplication {
        client: test_client,
    });
    // The despawn lands between the trigger and the send that carries the resend.
    server_app.world_mut().despawn(server_entity);

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();
    server_app.exchange_with_client(&mut client_app);

    assert!(
        !client_app
            .world()
            .resource::<ServerEntityMap>()
            .to_client()
            .contains_key(&server_entity),
        "the despawn must reach the client despite the resend"
    );
}

#[test]
fn keeps_removal_made_before_resend_is_sent() {
    let mut server_app = App::new();
    let mut client_app = App::new();
    for app in [&mut server_app, &mut client_app] {
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            RepliconPlugins.set(ServerPlugin::new(PostUpdate)),
        ))
        .replicate::<Counter>()
        .replicate::<Flag>()
        .finish();
    }

    server_app.connect_client(&mut client_app);

    let server_entity = server_app
        .world_mut()
        .spawn((Replicated, Counter(2), Flag))
        .id();

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();
    server_app.exchange_with_client(&mut client_app);

    let client_entity = *client_app
        .world()
        .resource::<ServerEntityMap>()
        .to_client()
        .get(&server_entity)
        .unwrap();
    assert!(client_app.world().get::<Flag>(client_entity).is_some());

    let test_client = **client_app.world().resource::<TestClientEntity>();
    server_app.world_mut().trigger(ResendReplication {
        client: test_client,
    });
    // The removal lands between the trigger and the send that carries the resend.
    server_app
        .world_mut()
        .entity_mut(server_entity)
        .remove::<Flag>();

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();
    server_app.exchange_with_client(&mut client_app);

    assert!(
        client_app.world().get::<Flag>(client_entity).is_none(),
        "the removal must reach the client despite the resend"
    );
    assert_eq!(
        client_app.world().get::<Counter>(client_entity),
        Some(&Counter(2))
    );
}

#[test]
fn resends_once() {
    let mut server_app = App::new();
    let mut client_app = App::new();
    for app in [&mut server_app, &mut client_app] {
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            RepliconPlugins.set(ServerPlugin::new(PostUpdate)),
        ))
        .replicate::<Counter>()
        .finish();
    }

    server_app.connect_client(&mut client_app);

    let server_entity = server_app.world_mut().spawn((Replicated, Counter(1))).id();

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();
    server_app.exchange_with_client(&mut client_app);

    let client_entity = *client_app
        .world()
        .resource::<ServerEntityMap>()
        .to_client()
        .get(&server_entity)
        .unwrap();

    let test_client = **client_app.world().resource::<TestClientEntity>();
    server_app.world_mut().trigger(ResendReplication {
        client: test_client,
    });

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();
    server_app.exchange_with_client(&mut client_app);

    // The client drifts after the resend has landed, and the server has nothing new to send.
    client_app
        .world_mut()
        .get_mut::<Counter>(client_entity)
        .unwrap()
        .0 = 7;

    server_app.update();
    server_app.exchange_with_client(&mut client_app);
    client_app.update();
    server_app.exchange_with_client(&mut client_app);

    assert_eq!(
        client_app.world().get::<Counter>(client_entity),
        Some(&Counter(7)),
        "a later send must carry only changes, not the whole world again"
    );
}

#[derive(Component, Deserialize, Serialize, Debug, PartialEq)]
struct Counter(u32);

#[derive(Component, Deserialize, Serialize)]
struct Flag;
