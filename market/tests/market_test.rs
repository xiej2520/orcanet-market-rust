use lib_proto::market::*;
use tonic::Request;

#[tokio::test]
async fn test_register_file() {
    let mut client = MarketClient::connect("http://127.0.0.1:50051")
        .await
        .unwrap();

    // Create a User and register a file
    let user = User {
        id: "12345".into(),
        name: "Shuai Mu".into(),
        ip: "123.456.789.0".into(),
        port: 8000,
        price: 100,
    };
    // Create a Register File Request with the user and file hash to be sent to the server
    let register_file_request = RegisterFileRequest {
        user: Some(user),
        // Note: in practice we should use hashing function, but for testing I hardcode the string
        file_hash: "ratcoin.js-hash".into(),
    };
    // Send the register file request to the server
    let register_response = client
        .register_file(Request::new(register_file_request))
        .await
        .unwrap();
    println!("Test 1 - RegisterFile Response: {:?}", register_response);
}

#[tokio::test]
async fn check_holders_registered() {
    // Test 2: Check the Holders of a Registered File
    let mut client = MarketClient::connect("http://127.0.0.1:50051")
        .await
        .unwrap();

    let id = "22222".to_owned();
    let name = "Shuai Mu2".to_owned();

    let user = User {
        id: id.clone(),
        name: name.clone(),
        ip: "223.456.789.0".into(),
        port: 8000,
        price: 100,
    };

    let file_hash = "ratcoin.js-hash".to_owned();
    let register_file_request = RegisterFileRequest {
        user: Some(user),
        file_hash: file_hash.clone(),
    };
    let register_response = client
        .register_file(Request::new(register_file_request))
        .await
        .unwrap();
    println!("Test 2 - RegisterFile Response: {:?}", register_response);

    let check_holders_request = CheckHoldersRequest {
        file_hash: file_hash.clone(),
    };

    // Send check holders request to the server and await response
    let check_holders_response = client
        .check_holders(Request::new(check_holders_request))
        .await
        .unwrap()
        .into_inner();

    println!(
        "Test 2 - CheckHolders Response: {:?}",
        check_holders_response
    );
    assert!(
        check_holders_response
            .holders
            .iter()
            .any(|u| u.id == id && u.name == name),
        "Test 2 Failed: User Shuai Mu2 should be a holder.\n"
    );
}

#[tokio::test]
async fn check_holders_unregistered() {
    // Test 3: Check the Holders of a NON-Registered File
    let mut client = MarketClient::connect("http://127.0.0.1:50051")
        .await
        .unwrap();

    let check_holders_request = CheckHoldersRequest {
        file_hash: "shuaimu.jpeg-hash".into(),
    };
    // Send check holders request to the server and await response
    let check_holders_response = client
        .check_holders(Request::new(check_holders_request))
        .await
        .unwrap()
        .into_inner();
    println!(
        "Test 3 - CheckHolders Response: {:?}",
        check_holders_response
    );
    assert!(
        check_holders_response.holders.is_empty(),
        "Test 3 Failed: There should be no holders for an unregistered file.\n"
    );
}

#[tokio::test]
async fn check_different_user() {
    // not sure if this is necessary
    // Test 4: Create separate user querying the same file
    let mut client = MarketClient::connect("http://127.0.0.1:50051")
        .await
        .unwrap();

    // add file to server from one user
    let id1 = "44444".to_owned();
    let user1 = User {
        id: id1.clone(),
        name: "Shuai4".to_owned(),
        ip: "444.456.789.0".into(),
        port: 8000,
        price: 100,
    };

    let file_hash = "ratcoin.js-hash-4".to_owned();

    let register_file_request = RegisterFileRequest {
        user: Some(user1),
        file_hash: file_hash.clone(),
    };
    let register_response = client
        .register_file(Request::new(register_file_request))
        .await
        .unwrap();
    println!("Test 4 - RegisterFile Response: {:?}", register_response);

    let _user2 = User {
        id: "98765".into(),
        name: "Shumai".into(),
        ip: "100.200.300.400".into(),
        port: 8001,
        price: 101,
    };
    // Create Check Holders Request for a Registered File
    let check_holders_request = CheckHoldersRequest {
        file_hash: file_hash.clone(),
    };
    // Send check holders request to the server and await response
    let check_holders_response = client
        .check_holders(Request::new(check_holders_request))
        .await
        .unwrap()
        .into_inner();
    println!(
        "Test 4 - CheckHolders Response: {:?}",
        check_holders_response
    );
    // Assert that the user with ID "44444" is indeed a holder of the file.
    assert!(
        check_holders_response.holders.iter().any(|u| u.id == id1),
        "Test 4 Failed: Expected to find user with ID '44444' as a holder of the file 'ratcoin.js-hash-4'\n"
    );
}

#[tokio::test]
async fn test_register_no_user() {
    // Test 5: Attempt to Register a File with No User
    let mut client = MarketClient::connect("http://127.0.0.1:50051")
        .await
        .unwrap();

    let register_file_request = RegisterFileRequest {
        user: None, // Deliberately not providing a user
        file_hash: "test-5-file-hash".into(),
    };
    let register_response = client
        .register_file(Request::new(register_file_request))
        .await;
    println!("Test 5 - Register Response: {:?}", register_response);
    assert!(
        register_response.is_err(),
        "Test 5 Failed: Registration of a file without a user should not succeed."
    );
}

#[tokio::test]
async fn verify_file_holder() {
    // Test 6: Register a File and Verify the Holder
    let mut client = MarketClient::connect("http://127.0.0.1:50051")
        .await
        .unwrap();
    let user_for_test_6 = User {
        id: "user6".into(),
        name: "Bob".into(),
        ip: "192.168.1.6".into(),
        port: 8002,
        price: 60,
    };
    let register_file_request = RegisterFileRequest {
        user: Some(user_for_test_6),
        file_hash: "file-hash-test-6".into(),
    };
    client
        .register_file(Request::new(register_file_request))
        .await
        .unwrap();
    // Checking holders of the file
    let check_holders_request = CheckHoldersRequest {
        file_hash: "file-hash-test-6".into(),
    };
    let check_holders_response = client
        .check_holders(Request::new(check_holders_request))
        .await
        .unwrap()
        .into_inner();
    println!(
        "Test 6 - Check Holders Response: {:?}",
        check_holders_response
    );
    assert!(
        check_holders_response
            .holders
            .iter()
            .any(|u| u.id == "user6"),
        "Test 6 Failed: User Six should be a holder of the file."
    );
}

#[tokio::test]
async fn test_nonexistent_file() {
    // Test 7: Check for Holders of a File that Doesn't Exist
    let mut client = MarketClient::connect("http://127.0.0.1:50051")
        .await
        .unwrap();
    let check_holders_request = CheckHoldersRequest {
        file_hash: "nonexistent-file-hash".into(),
    };
    let check_holders_response = client
        .check_holders(Request::new(check_holders_request))
        .await
        .unwrap()
        .into_inner();
    println!(
        "Test 7 - Check Holders Response: {:?}",
        check_holders_response
    );
    assert!(
        check_holders_response.holders.is_empty(),
        "Test 7 Failed: A nonexistent file should have no holders."
    );
    println!("Test 7 - SUCCESS\n");
}
