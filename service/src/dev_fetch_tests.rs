use super::*;

const HASH: &str = "1234567890abcdef1234567890abcdef12345678";
const CAPS: &str = "multi_ack multi_ack_detailed side-band side-band-64k thin-pack ofs-delta no-progress agent=git/2.43 object-format=sha1";
fn repository() -> Repository {
    Repository {
        url: "https://github.com/team/repo.git".into(),
        reference: Some("main".into()),
    }
}
fn pkt(data: &[u8]) -> Vec<u8> {
    let mut out = format!("{:04x}", data.len() + 4).into_bytes();
    out.extend_from_slice(data);
    out
}
fn ad(caps: &str) -> Vec<u8> {
    let mut out = pkt(b"# service=git-upload-pack\n");
    out.extend_from_slice(b"0000");
    out.extend(pkt(format!("{HASH} HEAD\0{caps}\n").as_bytes()));
    out.extend(pkt(format!("{HASH} refs/heads/main\n").as_bytes()));
    out.extend_from_slice(b"0000");
    out
}
fn branch_advertisement() -> Vec<u8> {
    let mut bytes = ad(CAPS);
    bytes.truncate(bytes.len() - 4);
    bytes.extend(pkt(
        b"abcdef1234567890abcdef1234567890abcdef12 refs/heads/feature/nested\n",
    ));
    bytes.extend_from_slice(b"0000");
    bytes
}
fn wants(caps: &str) -> Vec<u8> {
    let suffix = if caps.is_empty() {
        String::new()
    } else {
        format!(" {caps}")
    };
    let mut out = pkt(format!("want {HASH}{suffix}\n").as_bytes());
    out.extend_from_slice(b"0000");
    out
}
fn pack(caps: &str) -> Vec<u8> {
    let mut out = wants(caps);
    out.extend(pkt(b"done\n"));
    out
}
fn shallow(pin: &str, lf: bool) -> Vec<u8> {
    let mut out = pkt(format!("shallow {pin}{}", if lf { "\n" } else { "" }).as_bytes());
    out.extend_from_slice(b"0000");
    out
}
fn pack_response() -> Vec<u8> {
    let mut out = pkt(b"NAK\n");
    out.extend(pkt(b"\x01PACKsynthetic"));
    out.extend_from_slice(b"0000");
    out
}
fn validate_request(
    body: &[u8],
    pin: &str,
    caps: &HashSet<String>,
) -> LocalResult<HashSet<String>> {
    validate_pack_request(body, pin, caps, &HashSet::from([HASH.into()]), Phase::Pack)
}
fn http(kind: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}
fn until() -> Instant {
    Instant::now() + Duration::from_secs(2)
}
fn response(bytes: &[u8], limit: usize) -> LocalResult<Vec<u8>> {
    run(
        until(),
        &AtomicBool::new(false),
        read_response(bytes, AD_TYPE, limit),
    )
}
fn addresses() -> Vec<SocketAddr> {
    vec!["140.82.112.3:443".parse().unwrap()]
}
fn fixture(
    session: &mut FetchSession,
    index: usize,
    advertisement: bool,
    body: &[u8],
    response: Vec<u8>,
) {
    session.synthetic.push_back(Synthetic {
        addresses: addresses(),
        request: wire_request(&session.entries[index].destination, advertisement, body),
        response,
    });
}
fn advertised() -> FetchSession {
    let mut s = FetchSession::new(vec![repository()]).unwrap();
    fixture(&mut s, 0, true, b"", http(AD_TYPE, &ad(CAPS)));
    assert_eq!(
        s.request(0, true, b"", until(), &AtomicBool::new(false))
            .unwrap()
            .0,
        AD_TYPE
    );
    s
}
fn complete() -> FetchSession {
    let mut s = advertised();
    let caps = "side-band-64k ofs-delta agent=git/2.43";

    let body = pack(caps);
    let response = pack_response();
    fixture(&mut s, 0, false, &body, http(PACK_TYPE, &response));
    assert_eq!(
        s.request(0, false, &body, until(), &AtomicBool::new(false))
            .unwrap()
            .1,
        response
    );
    s
}

#[test]
fn repository_allowlist() {
    for url in [
        "https://github.com/team/repo.git",
        "https://EXAMPLE.ORG:443/a~b/c-d_1.git",
    ] {
        let mut r = repository();
        r.url = url.into();
        assert!(validate_repository(&r).is_ok(), "{url}");
    }
}
#[test]
fn repository_rejects_authority_and_path_escape() {
    for url in [
        "http://github.com/a",
        "https://github.com",
        "https://github.com/",
        "https://github.com//a",
        "https://github.com/a/",
        "https://github.com/a/../b",
        "https://github.com/./b",
        "https://github.com/a%2fb",
        "https://github.com/a?b",
        "https://github.com/a#b",
        "https://u:p@github.com/a",
        "https://github.com:444/a",
        "https://github.com:0443/a",
        "https://localhost/a",
        "https://host.local/a",
        "https://host.internal/a",
        "https://127.0.0.1/a",
        "https://2130706433/a",
        "https://127.1/a",
        "https://[::1]/a",
        "https://foo.123/a",
        "https://github.com./a",
        "https://-foo.com/a",
        "https://foo..com/a",
        "https://foo_bar.com/a",
        "https://github.com/a\\b",
        "https://github.com/a\r\nX: a",
        "https://github.com/é",
        "file:///a",
        "https://github.com/a b",
        "https://github.com/a;b",
        "https://foo.onion/a",
    ] {
        let mut r = repository();
        r.url = url.into();
        assert!(validate_repository(&r).is_err(), "{url}");
    }
}
#[test]
fn branches_are_narrow_git_refs() {
    for branch in [
        "main",
        "release/1.2",
        "feature/a_b-c",
        "refs/heads/main",
        "refs/heads/feature/nested",
    ] {
        assert!(safe_branch(branch));
    }
    for branch in [
        "", ".foo", "a/", "/a", "a//b", "a..b", "a.lock", "a/b.lock", "a.", "a@{b", "a~1", "a^",
        "a:b", "a?b", "a*b", "a[b", "a\\b", "a b", "a\nb", "-option", "HEAD",
    ] {
        assert!(!safe_branch(branch), "{branch}");
    }
    assert!(!safe_branch(&"a".repeat(256)));
}
#[test]
fn address_classes_fail_closed() {
    for address in [
        "0.1.2.3",
        "10.0.0.1",
        "100.64.0.1",
        "100.127.255.255",
        "127.0.0.1",
        "169.254.169.254",
        "172.16.0.1",
        "172.31.255.255",
        "192.0.0.9",
        "192.0.2.1",
        "192.52.193.1",
        "192.88.99.1",
        "192.168.1.1",
        "198.18.0.1",
        "198.19.255.255",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "239.1.1.1",
        "240.0.0.1",
        "255.255.255.255",
        "::",
        "::1",
        "::ffff:8.8.8.8",
        "::ffff:127.0.0.1",
        "::8.8.8.8",
        "64:ff9b::808:808",
        "64:ff9b:1::1",
        "100::1",
        "fc00::1",
        "fd00::1",
        "fe80::1",
        "ff02::1",
        "2001::1",
        "2001:20::1",
        "2001:2::1",
        "2001:db8::1",
        "2002:808:808::1",
        "2001:4860::5efe:808:808",
        "2001:4860::200:5efe:808:808",
        "3ffe::1",
        "3fff::1",
        "3fff:fff::1",
    ] {
        assert!(!public_address(address.parse().unwrap()), "{address}");
    }
    for address in [
        "8.8.8.8",
        "1.1.1.1",
        "140.82.112.3",
        "100.128.0.1",
        "172.32.0.1",
        "2606:4700:4700::1111",
        "2001:4860:4860::8888",
    ] {
        assert!(public_address(address.parse().unwrap()), "{address}");
    }
}
#[test]
fn resolver_answer_count_port_and_mixed_results() {
    assert!(approve_addresses(vec![]).is_err());
    assert!(approve_addresses(vec![addresses()[0]; 9]).is_err());
    assert_eq!(approve_addresses(vec![addresses()[0]; 8]).unwrap().len(), 1);
    let mut mixed = addresses();
    mixed.push("127.0.0.1:443".parse().unwrap());
    assert!(approve_addresses(mixed).is_err());
    assert!(approve_addresses(vec!["8.8.8.8:80".parse().unwrap()]).is_err());
    assert!(
        approve_addresses(vec![SocketAddr::V6(std::net::SocketAddrV6::new(
            "2606:4700::1".parse().unwrap(),
            443,
            0,
            1
        ))])
        .is_err()
    );
}
#[test]
fn advertisement_pins_exact_branch_or_head() {
    let parsed = parse_advertisement(&repository(), &ad(CAPS)).unwrap();
    assert_eq!(parsed.pin.reference, "refs/heads/main");
    assert_eq!(parsed.pin.commit, HASH);
    assert!(!parsed.capabilities.contains("shallow"));
    let mut repo = repository();
    repo.reference = None;
    assert_eq!(
        parse_advertisement(&repo, &ad(CAPS)).unwrap().pin.reference,
        "HEAD"
    );
    repo.reference = Some("missing".into());
    assert!(parse_advertisement(&repo, &ad(CAPS)).is_err());
}
#[test]
fn advertisement_normalizes_bare_and_qualified_branches() {
    for (input, expected, commit) in [
        (Some("main"), "refs/heads/main", HASH),
        (Some("refs/heads/main"), "refs/heads/main", HASH),
        (
            Some("feature/nested"),
            "refs/heads/feature/nested",
            "abcdef1234567890abcdef1234567890abcdef12",
        ),
        (
            Some("refs/heads/feature/nested"),
            "refs/heads/feature/nested",
            "abcdef1234567890abcdef1234567890abcdef12",
        ),
        (None, "HEAD", HASH),
    ] {
        let mut repo = repository();
        repo.reference = input.map(str::to_owned);
        validate_repository(&repo).unwrap();
        let checkout = parse_advertisement(&repo, &branch_advertisement())
            .unwrap()
            .pin;
        assert_eq!(checkout.reference, expected, "{input:?}");
        assert_eq!(checkout.commit, commit, "{input:?}");

        let mut session = FetchSession::new(vec![repo]).unwrap();
        fixture(
            &mut session,
            0,
            true,
            b"",
            http(AD_TYPE, &branch_advertisement()),
        );
        session
            .request(0, true, b"", until(), &AtomicBool::new(false))
            .unwrap();

        let body = pack("side-band-64k");
        let body = String::from_utf8(body)
            .unwrap()
            .replace(HASH, commit)
            .into_bytes();
        fixture(
            &mut session,
            0,
            false,
            &body,
            http(PACK_TYPE, &pack_response()),
        );
        session
            .request(0, false, &body, until(), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(session.pinned_checkouts().unwrap(), vec![checkout.clone()]);
        session.verify_checkouts(&[checkout]).unwrap();
        assert!(session.sealed);
        assert_eq!(session.requests, 2);
        assert!(session.synthetic.is_empty());
    }
}
#[test]
fn missing_exact_branch_never_falls_back_and_seals_session() {
    let mut bytes = branch_advertisement();
    bytes.truncate(bytes.len() - 4);
    for name in [
        "refs/tags/missing",
        "refs/heads/missing/child",
        "refs/heads/refs/heads/missing",
    ] {
        bytes.extend(pkt(format!("{HASH} {name}\n").as_bytes()));
    }
    bytes.extend_from_slice(b"0000");
    for input in ["missing", "refs/heads/missing", "refs/heads/feature/absent"] {
        let mut repo = repository();
        repo.reference = Some(input.into());
        validate_repository(&repo).unwrap();
        assert!(parse_advertisement(&repo, &bytes).is_err(), "{input}");
        let mut session = FetchSession::new(vec![repo]).unwrap();
        fixture(&mut session, 0, true, b"", http(AD_TYPE, &bytes));
        assert!(session
            .request(0, true, b"", until(), &AtomicBool::new(false))
            .is_err());
        assert!(session.sealed);
        assert!(session.pinned_checkouts().is_err());
        fixture(&mut session, 0, true, b"", http(AD_TYPE, &ad(CAPS)));
        assert!(session
            .request(0, true, b"", until(), &AtomicBool::new(false))
            .is_err());
        assert_eq!(session.requests, 1);
        assert_eq!(session.synthetic.len(), 1);
    }
}
#[test]
fn advertisement_no_fallback_or_sha256() {
    for caps in [
        "shallow object-format=sha256",
        "shallow object-format=other",
        "shallow object-format",
        "multi_ack",
        "shallow shallow",
        "shallow  agent=x",
        "shallow\nagent=x",
    ] {
        assert!(
            parse_advertisement(&repository(), &ad(caps)).is_err(),
            "{caps}"
        );
    }
    let mut bytes = ad(CAPS);
    bytes.extend_from_slice(b"0000");
    assert!(parse_advertisement(&repository(), &bytes).is_err());
    assert!(parse_advertisement(&repository(), &vec![b'0'; AD_LIMIT + 1]).is_err());
}
#[test]
fn advertisement_duplicate_refs_late_caps_and_zero_oid() {
    for record in [
        format!("{HASH} refs/heads/main\n"),
        format!("{HASH} refs/heads/other\0shallow\n"),
        format!("{} refs/heads/other\n", "0".repeat(40)),
        format!("{HASH} refs/heads/a..b\n"),
    ] {
        let mut bytes = ad(CAPS);
        bytes.truncate(bytes.len() - 4);
        bytes.extend(pkt(record.as_bytes()));
        bytes.extend_from_slice(b"0000");
        assert!(
            parse_advertisement(&repository(), &bytes).is_err(),
            "{record}"
        );
    }
}
#[test]
fn packet_malformed_and_truncated() {
    for data in [
        b"".as_slice(),
        b"000",
        b"zzzz",
        b"0001",
        b"0002",
        b"0003",
        b"ffff",
        b"0008abc",
    ] {
        let mut bytes = data;
        assert!(packet(&mut bytes).is_err(), "{data:?}");
    }
    let mut bytes = b"0004".as_slice();
    assert_eq!(packet(&mut bytes).unwrap(), Some(b"".as_slice()));
}
#[test]
fn every_advertisement_truncation_fails() {
    let bytes = ad(CAPS);
    for i in 0..bytes.len() {
        assert!(
            parse_advertisement(&repository(), &bytes[..i]).is_err(),
            "{i}"
        );
    }
}
#[test]
fn safe_pack_capabilities_and_optional_final_flush() {
    let caps = parse_advertisement(&repository(), &ad(CAPS))
        .unwrap()
        .capabilities;
    for selected in [
        "side-band-64k",
        "multi_ack side-band thin-pack ofs-delta no-progress agent=git/2.43 object-format=sha1",
        "multi_ack_detailed side-band-64k",
    ] {
        let mut bytes = pack(selected);
        assert!(validate_request(&bytes, HASH, &caps).is_ok());
        bytes.extend_from_slice(b"0000");
        assert!(validate_request(&bytes, HASH, &caps).is_ok());
        assert!(validate_request(&wants(selected), HASH, &caps).is_err());
    }
    assert!(validate_request(&pack(""), HASH, &caps).is_err());
}
#[test]
fn pack_extensions_and_capability_conflicts_fail() {
    let caps = parse_advertisement(&repository(), &ad(CAPS))
        .unwrap()
        .capabilities;
    for selected in [
        "shallow",
        "filter",
        "packfile-uris=https",
        "include-tag",
        "object-format=sha256",
        "agent=",
        "agent=foo:bar",
        "agent=foo\nbar",
        "multi_ack multi_ack_detailed",
        "side-band side-band-64k",
        "ofs-delta ofs-delta",
        "unknown",
        "thin-pack=1",
    ] {
        assert!(
            validate_request(&pack(&format!("side-band-64k {selected}")), HASH, &caps).is_err(),
            "{selected}"
        );
    }
    assert!(validate_request(&pack("side-band-64k agent=git/2"), HASH, &HashSet::new()).is_err());
    assert!(validate_request(
        &pack(&format!("side-band-64k agent={}", "x".repeat(129))),
        HASH,
        &caps
    )
    .is_err());
}
#[test]
fn pack_exact_want_and_no_haves() {
    let caps = parse_advertisement(&repository(), &ad(CAPS))
        .unwrap()
        .capabilities;
    let good = pack("side-band-64k");
    for i in 0..good.len() {
        // A terminal LF in the done payload is optional, not its pkt-line length.
        assert!(validate_request(&good[..i], HASH, &caps).is_err());
    }
    for data in [
        b"have abc\n".as_slice(),
        b"filter blob:none\n",
        b"want abc\n",
        b"deepen 2\n",
    ] {
        let mut bytes = good.clone();
        bytes.extend(pkt(data));
        assert!(validate_request(&bytes, HASH, &caps).is_err());
    }
    let other = "abcdef1234567890abcdef1234567890abcdef12";
    assert!(validate_request(&good, other, &caps).is_err());
    assert!(validate_request(&vec![0; REQUEST_LIMIT + 1], HASH, &caps).is_err());
}
#[test]
fn fixed_wire_requests_do_not_use_worker_authority() {
    let d = destination(&repository()).unwrap();
    assert_eq!(d.host, "github.com");
    let get = String::from_utf8(wire_request(&d, true, b"")).unwrap();
    assert!(get.starts_with("GET /team/repo.git/info/refs?service=git-upload-pack HTTP/1.1\r\n"));
    assert!(get.contains("Host: github.com\r\n"));
    for forbidden in ["Authorization:", "Cookie:", "Git-Protocol:", "Proxy-"] {
        assert!(!get.contains(forbidden));
    }
    let post = wire_request(&d, false, b"abc");
    assert!(post.ends_with(b"\r\n\r\nabc"));
    assert!(post.starts_with(b"POST /team/repo.git/git-upload-pack HTTP/1.1\r\n"));
}
#[test]
fn exact_http_types_and_status() {
    assert_eq!(response(&http(AD_TYPE, b"test"), 4).unwrap(), b"test");
    for kind in [
        "text/plain",
        PACK_TYPE,
        "application/x-git-upload-pack-advertisement; charset=utf-8",
    ] {
        assert!(response(&http(kind, b""), 4).is_err());
    }
    for status in [
        "301 Redirect",
        "401 Unauthorized",
        "100 Continue",
        "101 Switching",
        "500 Error",
        "2000 Bad",
    ] {
        let bytes =
            format!("HTTP/1.1 {status}\r\nContent-Type: {AD_TYPE}\r\nContent-Length: 0\r\n\r\n");
        assert!(response(bytes.as_bytes(), 4).is_err());
    }
}
#[test]
fn http_header_framing_rejections() {
    for extra in [
        "Content-Encoding: identity\r\n",
        "Trailer: X\r\n",
        "Content-Length: 1\r\nContent-Length: 1\r\n",
        "Content-Length: +1\r\n",
        "Content-Length: 1, 1\r\n",
        "Content-Length: 999999999999999999999999999999\r\n",
        "Content-Length: 5\r\n",
        "Content-Length: 1\r\nTransfer-Encoding: chunked\r\n",
        "Transfer-Encoding: gzip\r\n",
        " Bad: folded\r\n",
        "Bad : value\r\n",
        "X: a\tb\r\n",
        "X: a\rX: b\r\n",
    ] {
        let bytes = format!("HTTP/1.1 200 OK\r\nContent-Type: {AD_TYPE}\r\n{extra}\r\nx");
        assert!(response(bytes.as_bytes(), 4).is_err(), "{extra:?}");
    }
}
#[test]
fn http_raw_header_budget_and_control() {
    let bytes = format!(
        "HTTP/1.1 200 OK\r\nX: {}\r\nContent-Type: {AD_TYPE}\r\n\r\n",
        " ".repeat(HEAD_LIMIT)
    );
    assert!(response(bytes.as_bytes(), 4).is_err());
    assert!(response(b"HTTP/1.1 200 OK\nContent-Type: x\n\n", 4).is_err());
    assert!(response(b"HTTP/1.1 200 OK\r\nContent-Type: x\0y\r\n\r\n", 4).is_err());
}
#[test]
fn http_length_body_budget_truncation_and_trailing_bytes() {
    assert!(response(&http(AD_TYPE, b"12345"), 4).is_err());
    let mut bytes = http(AD_TYPE, b"abc");
    bytes.pop();
    assert!(response(&bytes, 4).is_err());
    let mut bytes = http(AD_TYPE, b"abc");
    bytes.push(b'x');
    assert!(response(&bytes, 4).is_err());
    let mut bytes = format!("HTTP/1.1 200 OK\r\nContent-Type: {AD_TYPE}\r\n\r\n").into_bytes();
    bytes.extend_from_slice(b"1234");
    assert_eq!(response(&bytes, 4).unwrap(), b"1234");
    bytes.push(b'5');
    assert!(response(&bytes, 4).is_err());
}
#[test]
fn http_chunked_bounds_and_no_trailers_or_extensions() {
    let head =
        format!("HTTP/1.1 200 OK\r\nContent-Type: {AD_TYPE}\r\nTransfer-Encoding: chunked\r\n\r\n");
    let valid = format!("{head}2\r\nab\r\n2\r\ncd\r\n0\r\n\r\n");
    assert_eq!(response(valid.as_bytes(), 4).unwrap(), b"abcd");
    for body in [
        "5\r\nabcde\r\n0\r\n\r\n",
        "1;foo=bar\r\nx\r\n0\r\n\r\n",
        "0\r\nX: y\r\n\r\n",
        "z\r\n",
        "1\r\nx",
        "0\r\n\r\nx",
    ] {
        assert!(
            response(format!("{head}{body}").as_bytes(), 4).is_err(),
            "{body:?}"
        );
    }
}
#[test]
fn session_advertisement_pack_verify_seals() {
    let mut s = complete();
    assert_eq!(s.requests, 2);
    assert_eq!(s.pack_bytes, pack_response().len());
    let checkouts = s.pinned_checkouts().unwrap();
    s.verify_checkouts(&checkouts).unwrap();
    assert!(s
        .request(0, true, b"", until(), &AtomicBool::new(false))
        .is_err());
    assert!(s.verify_checkouts(&checkouts).is_err());
}
#[test]
fn session_wrong_checkout_seals_irreversibly() {
    for field in 0..3 {
        let mut s = complete();
        let mut checkouts = s.pinned_checkouts().unwrap();
        match field {
            0 => checkouts[0].url.push('x'),
            1 => checkouts[0].reference.push('x'),
            _ => checkouts[0].commit.push('x'),
        }
        assert!(s.verify_checkouts(&checkouts).is_err());
        assert!(s.sealed);
        assert!(s.verify_checkouts(&s.pinned_checkouts().unwrap()).is_err());
    }
}
#[test]
fn session_empty_and_premature_verification() {
    let mut s = FetchSession::new(vec![]).unwrap();
    s.verify_checkouts(&[]).unwrap();
    assert!(s.sealed);
    let mut s = advertised();
    let c = s.pinned_checkouts().unwrap();
    assert!(s.verify_checkouts(&c).is_err());
    assert!(s.sealed);
    let mut s = complete();
    assert!(s.verify_checkouts(&[]).is_err());
    assert!(s.sealed);
}
#[test]
fn phases_and_failure_never_retry() {
    let mut s = FetchSession::new(vec![repository()]).unwrap();
    assert!(s.pinned_checkouts().is_err());
    assert!(s
        .request(0, false, &pack(""), until(), &AtomicBool::new(false))
        .is_err());
    assert!(s.sealed);
    let mut s = advertised();
    assert!(s
        .request(0, true, b"", until(), &AtomicBool::new(false))
        .is_err());
    assert!(s.sealed);
    let mut s = complete();
    assert!(s
        .request(0, false, &pack(""), until(), &AtomicBool::new(false))
        .is_err());
    assert!(s.sealed);
}
#[test]
fn invalid_index_or_ad_body_seals() {
    for (index, body) in [(1, b"".as_slice()), (0, b"x".as_slice())] {
        let mut s = FetchSession::new(vec![repository()]).unwrap();
        assert!(s
            .request(index, true, body, until(), &AtomicBool::new(false))
            .is_err());
        assert!(s.sealed);
    }
}
#[test]
fn synthetic_transport_never_falls_back_live() {
    let mut s = FetchSession::new(vec![repository()]).unwrap();
    assert_eq!(
        s.request(0, true, b"", until(), &AtomicBool::new(false))
            .unwrap_err(),
        UNAVAILABLE
    );
    assert!(s.sealed);
}
#[test]
fn synthetic_private_dns_rejected_before_response() {
    let mut s = FetchSession::new(vec![repository()]).unwrap();
    fixture(&mut s, 0, true, b"", http(AD_TYPE, &ad(CAPS)));
    s.synthetic[0].addresses = vec!["127.0.0.1:443".parse().unwrap()];
    assert!(s
        .request(0, true, b"", until(), &AtomicBool::new(false))
        .is_err());
    assert!(s.sealed);
}
#[test]
fn job_wide_pack_budget_not_per_repository() {
    let p = pack("side-band-64k");
    let response = pack_response();
    for excess in [0, 1] {
        let mut s = advertised();
        s.pack_bytes = PACK_LIMIT - response.len() + excess;
        fixture(&mut s, 0, false, &p, http(PACK_TYPE, &response));
        let result = s.request(0, false, &p, until(), &AtomicBool::new(false));
        if excess == 0 {
            result.unwrap();
            assert_eq!(s.pack_bytes, PACK_LIMIT);
        } else {
            assert!(result.is_err());
            assert!(s.sealed);
        }
    }
}

#[test]
fn job_wide_request_cap() {
    let mut s = FetchSession::new(vec![repository()]).unwrap();
    s.requests = REQUESTS;
    assert!(s
        .request(0, true, b"", until(), &AtomicBool::new(false))
        .is_err());
    assert!(s.sealed);
}
#[test]
fn cancellation_deadline_and_async_stall_are_bounded() {
    let mut s = FetchSession::new(vec![repository()]).unwrap();
    assert_eq!(
        s.request(0, true, b"", until(), &AtomicBool::new(true))
            .unwrap_err(),
        LocalError::Interrupted
    );
    assert!(s.sealed);
    let mut s = FetchSession::new(vec![repository()]).unwrap();
    assert_eq!(
        s.request(0, true, b"", Instant::now(), &AtomicBool::new(false))
            .unwrap_err(),
        LocalError::Deadline
    );
    let cancel = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(Duration::from_millis(30));
            cancel.store(true, Ordering::SeqCst);
        });
        let result: LocalResult<()> = run(until(), &cancel, std::future::pending());
        assert_eq!(result, Err(LocalError::Interrupted));
    });
    let result: LocalResult<()> = run(
        Instant::now() + Duration::from_millis(20),
        &AtomicBool::new(false),
        std::future::pending(),
    );
    assert_eq!(result, Err(LocalError::Deadline));
}
#[test]
fn pack_negotiation_shape_is_exact() {
    let caps = parse_advertisement(&repository(), &ad(CAPS))
        .unwrap()
        .capabilities;
    for middle in [
        pkt(b"deepen 2\n"),
        pkt(b"deepen 0\n"),
        pkt(b"have 123\n"),
        pkt(b"filter blob:none\n"),
        pkt(b"want abc\n"),
    ] {
        let mut body = pkt(format!("want {HASH} side-band-64k\n").as_bytes());
        body.extend(middle);
        body.extend_from_slice(b"0000");
        body.extend(pkt(b"done\n"));
        assert!(validate_request(&body, HASH, &caps).is_err());
    }
    let mut body = pack("side-band-64k");
    body.extend_from_slice(b"00000000");
    assert!(validate_request(&body, HASH, &caps).is_err());
}
#[test]
fn ten_repositories_fit_without_expanding_the_request_budget() {
    for count in [11, 16, 17] {
        assert!(FetchSession::new(vec![repository(); count]).is_err());
    }
    let mut s = FetchSession::new(vec![repository(); 10]).unwrap();
    for index in 0..10 {
        fixture(&mut s, index, true, b"", http(AD_TYPE, &ad(CAPS)));
        s.request(index, true, b"", until(), &AtomicBool::new(false))
            .unwrap();

        let p = pack("side-band-64k");
        fixture(&mut s, index, false, &p, http(PACK_TYPE, &pack_response()));
        s.request(index, false, &p, until(), &AtomicBool::new(false))
            .unwrap();
    }
    assert_eq!(s.requests, 20);
    s.verify_checkouts(&s.pinned_checkouts().unwrap()).unwrap();
}
#[test]
fn server_failure_latches_even_with_valid_retry_queued() {
    let mut s = FetchSession::new(vec![repository()]).unwrap();
    fixture(
        &mut s,
        0,
        true,
        b"",
        b"HTTP/1.1 302 Redirect\r\nLocation: https://other.org/\r\n\r\n".to_vec(),
    );
    fixture(&mut s, 0, true, b"", http(AD_TYPE, &ad(CAPS)));
    assert!(s
        .request(0, true, b"", until(), &AtomicBool::new(false))
        .is_err());
    assert!(s
        .request(0, true, b"", until(), &AtomicBool::new(false))
        .is_err());
    assert_eq!(s.synthetic.len(), 1);
}
#[test]
fn dns_addresses_are_frozen_after_advertisement() {
    let mut s = advertised();
    let approved = s.entries[0].addresses.clone().unwrap();
    let p = pack("side-band-64k");
    fixture(&mut s, 0, false, &p, http(PACK_TYPE, &pack_response()));
    s.synthetic[0].addresses = vec!["127.0.0.1:443".parse().unwrap()];
    s.request(0, false, &p, until(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(s.entries[0].addresses.as_ref(), Some(&approved));
}
#[test]
fn multiple_repositories_preserve_order_and_identity() {
    let mut second = repository();
    second.url = "https://example.org/another".into();
    second.reference = None;
    let mut s = FetchSession::new(vec![repository(), second.clone()]).unwrap();
    for i in [1, 0] {
        fixture(&mut s, i, true, b"", http(AD_TYPE, &ad(CAPS)));
        s.request(i, true, b"", until(), &AtomicBool::new(false))
            .unwrap();

        let p = pack("side-band-64k");
        fixture(&mut s, i, false, &p, http(PACK_TYPE, &pack_response()));
        s.request(i, false, &p, until(), &AtomicBool::new(false))
            .unwrap();
    }
    let c = s.pinned_checkouts().unwrap();
    assert_eq!(c[0].url, repository().url);
    assert_eq!(c[1].url, second.url);
    assert_eq!(c[1].reference, "HEAD");
    s.verify_checkouts(&c).unwrap();
}

#[test]
fn worker_advertisement_is_a_positive_upstream_subset_and_preserves_refs() {
    let upstream = format!("{CAPS} shallow symref=HEAD:refs/heads/main include-tag no-done deepen-since deepen-not deepen-relative filter packfile-uris=https unknown=value");
    let mut bytes = ad(&upstream);
    bytes.truncate(bytes.len() - 4);
    let tag_records = [
        format!("{HASH} refs/tags/v1\n"),
        format!("{HASH} refs/tags/v1^{{}}\n"),
    ];
    for record in &tag_records {
        bytes.extend(pkt(record.as_bytes()));
    }
    bytes.extend_from_slice(b"0000");
    let parsed = parse_advertisement(&repository(), &bytes).unwrap();
    let expected = format!("{CAPS} symref=HEAD:refs/heads/main");
    let mut expected = ad(&expected);
    expected.truncate(expected.len() - 4);
    for record in tag_records {
        expected.extend(pkt(record.as_bytes()));
    }
    expected.extend_from_slice(b"0000");
    assert_eq!(parsed.bytes, expected);
    assert_eq!(
        parse_advertisement(&repository(), &parsed.bytes)
            .unwrap()
            .capabilities,
        parsed.capabilities
    );

    let minimal = "side-band";
    let parsed = parse_advertisement(&repository(), &ad(minimal)).unwrap();
    assert_eq!(parsed.bytes, ad(minimal));
    assert_eq!(parsed.capabilities, HashSet::from(["side-band".into()]));
    for caps in [
        "side-band-64k",
        "side-band no-progress",
        "side-band agent=git/2",
        "side-band object-format=sha1",
        "side-band include-tag",
    ] {
        assert!(
            validate_request(&pack(caps), HASH, &parsed.capabilities).is_err(),
            "{caps}"
        );
    }
}
#[test]
fn advertisement_retained_parameters_are_unique_safe_and_consistent() {
    for extra in [
        "agent",
        "agent=",
        "agent=foo:bar",
        "agent=a=b",
        "agent=git/2 agent=git/3",
        "object-format=sha1 object-format=sha256",
        "object-format=sha1 object-format=sha1",
        "symref",
        "symref=HEAD:",
        "symref=other:refs/heads/main",
        "symref=HEAD:refs/heads/missing",
        "symref=HEAD:refs/heads/main symref=HEAD:refs/heads/main",
        "symref=HEAD:refs/heads/a..b",
        "side-band-64k=1",
        "unknown=x unknown=y",
    ] {
        let caps = format!("side-band-64k {extra}");
        assert!(
            parse_advertisement(&repository(), &ad(&caps)).is_err(),
            "{extra}"
        );
    }
    let caps = format!("side-band agent={}", "x".repeat(129));
    assert!(parse_advertisement(&repository(), &ad(&caps)).is_err());
    assert!(parse_advertisement(&repository(), &ad("shallow agent=git/2")).is_err());
    let caps = "shallow side-band symref=HEAD:refs/heads/main";
    let mut mismatched = pkt(b"# service=git-upload-pack\n");
    mismatched.extend_from_slice(b"0000");
    mismatched.extend(pkt(format!(
        "abcdef1234567890abcdef1234567890abcdef12 HEAD\0{caps}\n"
    )
    .as_bytes()));
    mismatched.extend(pkt(format!("{HASH} refs/heads/main\n").as_bytes()));
    mismatched.extend_from_slice(b"0000");
    assert!(parse_advertisement(&repository(), &mismatched).is_err());
}
#[test]
fn dropped_upstream_capabilities_never_authorize_client_use() {
    let mut s = FetchSession::new(vec![repository()]).unwrap();
    let caps =
        format!("{CAPS} include-tag no-done deepen-since deepen-not filter packfile-uris=https");
    fixture(&mut s, 0, true, b"", http(AD_TYPE, &ad(&caps)));
    let (_, bytes) = s
        .request(0, true, b"", until(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!(bytes, ad(CAPS));
    for dropped in [
        "include-tag",
        "no-done",
        "deepen-since",
        "deepen-not",
        "filter",
        "packfile-uris=https",
    ] {
        assert!(!s.entries[0].capabilities.contains(dropped));
        assert!(validate_pack_request(
            &pack(&format!("side-band-64k {dropped}")),
            HASH,
            &s.entries[0].capabilities,
            &HashSet::from([HASH.into()]),
            Phase::Pack
        )
        .is_err());
    }
}
fn request_fields(wants: &[&[u8]], done: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    for want in wants {
        body.extend(pkt(want));
    }
    body.extend_from_slice(b"0000");
    body.extend(pkt(done));
    body
}
#[test]
fn multiple_advertised_wants_include_pin_not_necessarily_first() {
    let other = "abcdef1234567890abcdef1234567890abcdef12";
    let ad = parse_advertisement(&repository(), &branch_advertisement()).unwrap();
    for lf in ["", "\n"] {
        let first = format!("want {other} side-band-64k{lf}");
        let pin = format!("want {HASH}{lf}");
        for done in [b"done".as_slice(), b"done\n"] {
            let body = request_fields(&[first.as_bytes(), pin.as_bytes()], done);
            validate_pack_request(&body, HASH, &ad.capabilities, &ad.advertised, Phase::Pack)
                .unwrap();
            let mut flushed = body.clone();
            flushed.extend_from_slice(b"0000");
            validate_pack_request(
                &flushed,
                HASH,
                &ad.capabilities,
                &ad.advertised,
                Phase::Pack,
            )
            .unwrap();
            let repeated =
                request_fields(&[first.as_bytes(), pin.as_bytes(), pin.as_bytes()], done);
            validate_pack_request(
                &repeated,
                HASH,
                &ad.capabilities,
                &ad.advertised,
                Phase::Pack,
            )
            .unwrap();
            for invalid in [
                request_fields(&[first.as_bytes()], done),
                request_fields(
                    &[
                        first.as_bytes(),
                        format!("want {HASH} ofs-delta").as_bytes(),
                    ],
                    done,
                ),
            ] {
                assert!(validate_pack_request(
                    &invalid,
                    HASH,
                    &ad.capabilities,
                    &ad.advertised,
                    Phase::Pack
                )
                .is_err());
            }
        }
    }
}
#[test]
fn request_text_controls_whitespace_and_extensions_fail_closed() {
    let caps = parse_advertisement(&repository(), &ad(CAPS))
        .unwrap()
        .capabilities;
    let want = format!("want {HASH} side-band-64k");
    for suffix in [
        "\r", "\r\n", "\n\n", "\nextra", "\t", "\0", " ", "  ", "\x7f",
    ] {
        for body in [
            request_fields(&[format!("{want}{suffix}").as_bytes()], b"done"),
            request_fields(&[want.as_bytes()], format!("done{suffix}").as_bytes()),
        ] {
            assert!(validate_request(&body, HASH, &caps).is_err());
        }
    }
    for middle in [
        "deepen 1".to_owned(),
        "deepen 2".into(),
        "deepen-since 1".into(),
        "deepen-not main".into(),
        "filter blob:none".into(),
        format!("shallow {HASH}"),
        format!("have {HASH}"),
        "want abc".into(),
        "want abcdef1234567890abcdef1234567890abcdef12".into(),
    ] {
        let mut s = advertised();
        let body = request_fields(&[want.as_bytes(), middle.as_bytes()], b"done");
        fixture(&mut s, 0, false, &body, http(PACK_TYPE, &pack_response()));
        assert!(s
            .request(0, false, &body, until(), &AtomicBool::new(false))
            .is_err());
        assert!(s.sealed);
        assert_eq!(s.synthetic.len(), 1, "reject before I/O: {middle}");
    }
    for body in [
        wants("side-band-64k"),
        request_fields(&[want.as_bytes()], b"done extra"),
    ] {
        assert!(validate_request(&body, HASH, &caps).is_err());
    }
}
fn sideband_response(nak: &[u8], packets: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = pkt(nak);
    for data in packets {
        bytes.extend(pkt(data));
    }
    bytes.extend_from_slice(b"0000");
    bytes
}
#[test]
fn final_response_accepts_fragmented_pack_signature_and_inert_progress() {
    for mode in ["side-band", "side-band-64k"] {
        let caps = HashSet::from([mode.to_owned()]);
        for lf in [false, true] {
            for split in 1..=5 {
                let mut packets = vec![b"\x02counting\r\n".to_vec()];
                for chunk in b"PACKsynthetic".chunks(split) {
                    let mut payload = vec![1];
                    payload.extend_from_slice(chunk);
                    packets.push(payload);
                    packets.push(b"\x02progress".to_vec());
                }
                let bytes = sideband_response(if lf { b"NAK\n" } else { b"NAK" }, &packets);
                validate_pack_response(&bytes, &caps).unwrap();
            }
        }
    }
}
#[test]
fn final_response_rejects_negotiation_errors_and_bad_sidebands() {
    let caps = HashSet::from(["side-band-64k".to_owned()]);
    let good = pack_response();
    for i in 0..good.len() {
        assert!(validate_pack_response(&good[..i], &caps).is_err(), "{i}");
    }
    let mut invalid = vec![b"PACKraw".to_vec(), shallow(HASH, true)];
    for nak in [
        b"ACK".as_slice(),
        b"ACK ready\n",
        b"ERR denied\n",
        b"NAK\r\n",
        b"NAK\n\n",
        b"NAK ",
        b" NAK",
    ] {
        invalid.push(sideband_response(nak, &[b"\x01PACKsynthetic".to_vec()]));
    }
    for packets in [
        vec![],
        vec![vec![]],
        vec![vec![1]],
        vec![vec![2]],
        vec![b"\x02progress".to_vec()],
        vec![b"\x03fatal".to_vec()],
        vec![b"\x00PACKx".to_vec()],
        vec![b"\x04PACKx".to_vec()],
        vec![b"\x01PACK".to_vec()],
        vec![b"\x01P".to_vec(), b"\x01AXKx".to_vec()],
        vec![b"\x01PACKx".to_vec(), b"\x03fatal".to_vec()],
    ] {
        invalid.push(sideband_response(b"NAK\n", &packets));
    }
    for extra in [b"0000".to_vec(), pkt(b"\x01PACKx"), b"x".to_vec()] {
        let mut bytes = good.clone();
        bytes.extend(extra);
        invalid.push(bytes);
    }
    let mut wrong_pin = shallow("abcdef1234567890abcdef1234567890abcdef12", true);
    wrong_pin.extend_from_slice(&good);
    invalid.push(wrong_pin);
    for bytes in invalid {
        assert!(validate_pack_response(&bytes, &caps).is_err());
        let mut s = advertised();

        let body = pack("side-band-64k");
        fixture(&mut s, 0, false, &body, http(PACK_TYPE, &bytes));
        assert!(s
            .request(0, false, &body, until(), &AtomicBool::new(false))
            .is_err());
        assert!(s.sealed);
        assert!(s.verify_checkouts(&s.pinned_checkouts().unwrap()).is_err());
    }
}
#[test]
fn sideband_packet_and_progress_budgets_are_enforced() {
    let large = HashSet::from(["side-band-64k".to_owned()]);
    let small = HashSet::from(["side-band".to_owned()]);
    let mut data = b"\x01PACK".to_vec();
    data.resize(997, b'x');
    let bytes = sideband_response(b"NAK", &[data]);
    assert!(validate_pack_response(&bytes, &small).is_err());
    validate_pack_response(&bytes, &large).unwrap();
    assert!(validate_pack_response(&bytes, &HashSet::new()).is_err());
    let mut packets = vec![b"\x01PACKx".to_vec()];
    for _ in 0..64 {
        let mut progress = vec![2];
        progress.resize(1025, b'p');
        packets.push(progress);
    }
    validate_pack_response(&sideband_response(b"NAK", &packets), &large).unwrap();
    packets.push(b"\x02x".to_vec());
    assert!(validate_pack_response(&sideband_response(b"NAK", &packets), &large).is_err());
}
mod executable {
    use super::*;
    use std::{
        fs,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        os::unix::{fs::DirBuilderExt, process::CommandExt},
        path::{Path, PathBuf},
        process::{Child, Command, Output, Stdio},
        sync::{Arc, Mutex},
        thread,
    };

    struct FiniteChild(Option<Child>);
    impl Drop for FiniteChild {
        fn drop(&mut self) {
            if let Some(child) = &mut self.0 {
                // Git's HTTP helper is in this fixture-owned process group too.
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    fn output(mut command: Command, input: &[u8]) -> Output {
        let child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .expect("start finite isolated Git fixture");
        let mut child = FiniteChild(Some(child));
        child
            .0
            .as_mut()
            .unwrap()
            .stdin
            .take()
            .unwrap()
            .write_all(input)
            .expect("write bounded Git fixture input");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if child
                .0
                .as_mut()
                .unwrap()
                .try_wait()
                .expect("poll Git fixture")
                .is_some()
            {
                return child
                    .0
                    .take()
                    .unwrap()
                    .wait_with_output()
                    .expect("collect Git fixture");
            }
            assert!(
                Instant::now() < deadline,
                "isolated Git fixture exceeded ten seconds"
            );
            thread::sleep(Duration::from_millis(2));
        }
    }
    fn checked(command: Command, input: &[u8]) -> Vec<u8> {
        let result = output(command, input);
        assert!(
            result.status.success(),
            "isolated Git fixture command failed (private stderr withheld)"
        );
        result.stdout
    }
    // Match dev_worker_local::git's policy; only private HOME/cwd differ.
    fn git(root: &Path) -> Command {
        let mut c = Command::new("/usr/bin/git");
        c.arg("--no-pager")
            .current_dir(root)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", root.join("home"))
            .env("XDG_CONFIG_HOME", root.join("home"))
            .env("LANG", "C.UTF-8")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "/bin/false")
            .env("SSH_ASKPASS", "/bin/false")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_LFS_SKIP_SMUDGE", "1")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_PROTOCOL_FROM_USER", "0")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@invalid")
            .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z");
        for setting in [
            "protocol.version=0",
            "protocol.allow=never",
            "protocol.http.allow=always",
            "credential.helper=",
            "core.askPass=/bin/false",
            "core.hooksPath=/dev/null",
            "core.fsmonitor=false",
            "core.attributesFile=/dev/null",
            "core.autocrlf=false",
            "core.quotePath=true",
            "http.followRedirects=false",
            "http.proxy=",
            "http.extraHeader=",
            "http.cookieFile=",
            "http.saveCookies=false",
            "http.sslVerify=true",
            "fetch.recurseSubmodules=false",
            "submodule.recurse=false",
            "filter.lfs.required=false",
            "filter.lfs.smudge=",
            "filter.lfs.clean=",
            "filter.lfs.process=",
        ] {
            c.arg("-c").arg(setting);
        }
        c
    }
    fn text(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes)
            .expect("ASCII Git fixture output")
            .trim_end_matches('\n')
            .to_owned()
    }
    struct GitFixture {
        root: PathBuf,
        repo: PathBuf,
        pin: String,
        tags: Vec<String>,
    }
    impl Drop for GitFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    impl GitFixture {
        fn new(shape: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("smith-dev-fetch-{:016x}", rand::random::<u64>()));
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&root)
                .expect("private Git fixture directory");
            let mut f = Self {
                repo: root.join("upstream.git"),
                root,
                pin: String::new(),
                tags: Vec::new(),
            };
            fs::create_dir(f.root.join("home")).expect("empty fixture HOME");
            let mut init = git(&f.root);
            init.args([
                "init",
                "--bare",
                "--template=",
                "--object-format=sha1",
                "--initial-branch=main",
            ])
            .arg(&f.repo);
            checked(init, b"");
            let root = f.commit("root", &[]);
            f.pin = match shape {
                "root" => root.clone(),
                "linear" => {
                    let middle = f.commit("middle", &[&root]);
                    f.commit("tip", &[&middle])
                }
                "merge" => {
                    let left = f.commit("left", &[&root]);
                    let right = f.commit("right", &[&root]);
                    f.commit("merge", &[&left, &right])
                }
                _ => panic!("unknown synthetic Git graph"),
            };
            f.run(&["update-ref", "refs/heads/main", &f.pin], b"");
            let divergent = f.commit("divergent", &[&root]);
            f.run(
                &["update-ref", "refs/heads/feature/nested", &divergent],
                b"",
            );
            f.run(&["tag", "lightweight-root", &root], b"");
            for (name, target) in [("annotated-tip", f.pin.clone()), ("annotated-root", root)] {
                f.run(&["tag", "-a", name, "-m", "fixture tag", &target], b"");
                f.tags.push(text(f.run(
                    &["rev-parse", "--verify", &format!("refs/tags/{name}")],
                    b"",
                )));
            }
            f
        }
        fn run(&self, args: &[&str], input: &[u8]) -> Vec<u8> {
            let mut c = git(&self.root);
            c.arg("--git-dir").arg(&self.repo).args(args);
            checked(c, input)
        }
        fn commit(&self, label: &str, parents: &[&str]) -> String {
            let blob = text(self.run(
                &["hash-object", "-w", "--stdin"],
                format!("{label} fixture\n").as_bytes(),
            ));
            let tree = text(self.run(
                &["mktree"],
                format!("100644 blob {blob}\tREADME.md\n").as_bytes(),
            ));
            let mut args = vec!["commit-tree", &tree, "-m", label];
            for parent in parents {
                args.extend(["-p", parent]);
            }
            text(self.run(&args, b""))
        }
    }
    fn split_wire(request: &[u8]) -> (bool, &[u8]) {
        let end = request
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("fixture HTTP head")
            + 4;
        let advertisement = request.starts_with(b"GET ");
        let body = &request[end..];
        assert_eq!(
            request,
            wire_request(&destination(&repository()).unwrap(), advertisement, body)
        );
        (advertisement, body)
    }
    fn response_lf(mut raw: &[u8], lf: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        assert_eq!(
            packet_text(packet(&mut raw).unwrap().unwrap()).unwrap(),
            "NAK"
        );
        append_packet(&mut bytes, if lf { b"NAK\n" } else { b"NAK" });
        bytes.extend_from_slice(raw);
        bytes
    }
    fn incoming(stream: &mut TcpStream) -> (bool, Vec<u8>) {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            assert!(header.len() < HEAD_LIMIT, "bounded fixture HTTP head");
            let mut byte = [0];
            stream
                .read_exact(&mut byte)
                .expect("fixture request header");
            header.push(byte[0]);
        }
        let header = std::str::from_utf8(&header).unwrap();
        let advertisement = match header.split("\r\n").next().unwrap() {
            "GET /repo/0.git/info/refs?service=git-upload-pack HTTP/1.1" => true,
            "POST /repo/0.git/git-upload-pack HTTP/1.1" => false,
            _ => panic!("unexpected fixture HTTP endpoint"),
        };
        let mut length = 0;
        for line in header.split("\r\n").skip(1).filter(|l| !l.is_empty()) {
            let (name, value) = line.split_once(':').unwrap();
            if name.eq_ignore_ascii_case("Content-Length") {
                length = value.trim().parse::<usize>().unwrap();
            }
            if name.eq_ignore_ascii_case("Git-Protocol") {
                panic!("worker must request Git v0");
            }
        }
        assert!(length <= REQUEST_LIMIT);
        assert!(!advertisement || length == 0);
        let mut body = vec![0; length];
        stream
            .read_exact(&mut body)
            .expect("bounded fixture HTTP body");
        (advertisement, body)
    }

    #[test]
    fn git_243_full_clones_through_actual_broker_with_pinned_checkout() {
        let mut version = Command::new("/usr/bin/git");
        version.env_clear().arg("--version");
        let version = text(checked(version, b""));
        assert_eq!(
            version, "git version 2.43.0",
            "fixture evidence is bound to the supported host Git"
        );
        for (shape, base, lf, corrupt) in [
            ("root", Some("main"), true, false),
            ("root", None, false, false),
            ("linear", Some("main"), false, false),
            ("linear", Some("refs/heads/main"), true, false),
            ("linear", None, true, false),
            ("merge", Some("main"), true, false),
            ("merge", Some("refs/heads/main"), false, false),
            ("merge", None, false, false),
            ("merge", Some("feature/nested"), true, false),
            ("linear", Some("refs/heads/feature/nested"), false, false),
            ("linear", Some("main"), true, true),
        ] {
            let started = Instant::now();
            let f = GitFixture::new(shape);
            let repo = Repository {
                reference: base.map(str::to_owned),
                ..repository()
            };
            let expected_pin =
                text(f.run(&["rev-parse", "--verify", &checkout_reference(base)], b""));
            let mut session = FetchSession::new(vec![repo.clone()]).unwrap();
            let root = f.root.clone();
            let upstream = f.repo.clone();
            let raw_ad = Arc::new(Mutex::new(Vec::new()));
            let capture = raw_ad.clone();
            session.local_transport = Some(Box::new(move |request| {
                let (advertisement, body) = split_wire(request);
                let mut command = git(&root);
                command.args(["upload-pack", "--stateless-rpc"]);
                if advertisement {
                    command.arg("--advertise-refs");
                }
                command.arg(&upstream);
                let raw = checked(command, body);
                let bytes = if advertisement {
                    let mut bytes = pkt(b"# service=git-upload-pack\n");
                    bytes.extend_from_slice(b"0000");
                    bytes.extend(raw);
                    *capture.lock().unwrap() = bytes.clone();
                    bytes
                } else if corrupt {
                    sideband_response(b"NAK\n", &[b"\x01PACKinvalid".to_vec()])
                } else {
                    response_lf(&raw, lf)
                };
                Ok(http(
                    if advertisement { AD_TYPE } else { PACK_TYPE },
                    &bytes,
                ))
            }));
            // Only an explicitly bound numeric loopback listener is reachable.
            // The broker's test transport above invokes local Git, never DNS/TLS.
            let listener = TcpListener::bind("127.0.0.1:0").expect("isolated loopback fixture");
            let address = listener.local_addr().unwrap();
            listener.set_nonblocking(true).unwrap();
            let server = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut seen = Vec::new();
                let mut worker_ad = Vec::new();
                while seen.len() < 2 {
                    assert!(Instant::now() < deadline, "loopback fixture deadline");
                    let (mut stream, peer) = match listener.accept() {
                        Ok(pair) => pair,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2));
                            continue;
                        }
                        Err(_) => panic!("loopback fixture accept"),
                    };
                    assert!(peer.ip().is_loopback());
                    stream.set_nonblocking(false).unwrap();
                    let (advertisement, body) = incoming(&mut stream);
                    let (kind, bytes) = session
                        .request(0, advertisement, &body, deadline, &AtomicBool::new(false))
                        .expect("actual Rust broker acquisition");
                    if advertisement {
                        worker_ad = bytes.clone();
                    }
                    seen.push((advertisement, body, bytes.len()));
                    stream
                        .write_all(&http(&kind, &bytes))
                        .expect("loopback fixture response");
                }
                (session, seen, worker_ad)
            });
            let clone_path = f.root.join("checkout");
            let mut clone = git(&f.root);
            clone.args(["clone", "--no-recurse-submodules", "--template="]);
            if let Some(base) = base {
                clone.arg("--branch").arg(branch_name(base));
            }
            clone
                .arg("--")
                .arg(format!("http://{address}/repo/0.git"))
                .arg(&clone_path);
            let result = output(clone, b"");
            let (mut session, seen, worker_ad) =
                server.join().expect("join isolated broker fixture");
            if corrupt {
                assert!(
                    !result.status.success(),
                    "native Git must reject a corrupt PACK"
                );
                assert_eq!(session.requests, 2);
                assert_eq!(seen.len(), 2);
                assert!(session.entries[0].phase == Phase::Complete);
                assert!(session.verify_checkouts(&[]).is_err());
                assert!(session.sealed);
                eprintln!(
                    "{version}: graph={shape}, corrupt PACK rejected by native Git, requests=2"
                );
                continue;
            }
            assert!(
                result.status.success(),
                "native clone failed (private stderr withheld)"
            );
            let raw_ad = raw_ad.lock().unwrap();
            let upstream_text = std::str::from_utf8(&raw_ad).unwrap();
            assert!(upstream_text.contains("include-tag"));
            assert!(upstream_text.contains("no-done"));
            let sanitized = std::str::from_utf8(&worker_ad).unwrap();
            for dropped in [
                "shallow",
                "include-tag",
                "no-done",
                "deepen-since",
                "deepen-not",
                "deepen-relative",
            ] {
                assert!(!sanitized.contains(dropped));
            }
            assert_eq!(seen.len(), 2);
            assert!(seen[0].0 && !seen[1].0);
            let post = std::str::from_utf8(&seen[1].1).unwrap();
            for forbidden in [
                "include-tag",
                "no-done",
                "shallow",
                "deepen",
                "have ",
                "filter ",
            ] {
                assert!(!post.contains(forbidden));
            }
            assert!(post.contains(&format!("want {expected_pin}")));
            assert_eq!(session.requests, 2);
            assert_eq!(session.pack_bytes, seen[1].2);
            assert!(session.entries[0].phase == Phase::Complete);
            let mut detach = git(&f.root);
            detach.arg("-C").arg(&clone_path).args([
                "checkout",
                "--detach",
                "--no-recurse-submodules",
                "HEAD",
                "--",
            ]);
            checked(detach, b"");
            let mut rev = git(&f.root);
            rev.arg("-C")
                .arg(&clone_path)
                .args(["rev-parse", "--verify", "HEAD^{commit}"]);
            let commit = text(checked(rev, b""));
            assert_eq!(commit, expected_pin);
            assert!(!clone_path.join(".git/shallow").exists());
            let cloned = |args: &[&str]| {
                let mut command = git(&f.root);
                command.arg("-C").arg(&clone_path).args(args);
                checked(command, b"")
            };
            assert_eq!(
                text(cloned(&["rev-parse", "--is-shallow-repository"])),
                "false"
            );
            // Ref traversal order can differ; compare complete identities/parents.
            let graph = |bytes: Vec<u8>| {
                let mut lines: Vec<_> = text(bytes).lines().map(str::to_owned).collect();
                lines.sort();
                lines
            };
            assert_eq!(
                graph(cloned(&["rev-list", "--all", "--parents"])),
                graph(f.run(&["rev-list", "--all", "--parents"], b""))
            );
            for branch in ["main", "feature/nested"] {
                let remote = format!("refs/remotes/origin/{branch}");
                let source = format!("refs/heads/{branch}");
                assert_eq!(
                    cloned(&["rev-parse", "--verify", &remote]),
                    f.run(&["rev-parse", "--verify", &source], b"")
                );
            }
            assert_eq!(
                cloned(&[
                    "for-each-ref",
                    "--format=%(refname) %(objectname)",
                    "refs/tags"
                ]),
                f.run(
                    &[
                        "for-each-ref",
                        "--format=%(refname) %(objectname)",
                        "refs/tags"
                    ],
                    b""
                )
            );
            for tag in &f.tags {
                cloned(&["cat-file", "-e", tag]);
            }
            let mut symbolic = git(&f.root);
            symbolic
                .arg("-C")
                .arg(&clone_path)
                .args(["symbolic-ref", "-q", "HEAD"]);
            assert!(!output(symbolic, b"").status.success());
            let checkout = Checkout {
                url: repo.url,
                reference: checkout_reference(repo.reference.as_deref()),
                commit,
            };
            assert_eq!(session.pinned_checkouts().unwrap(), vec![checkout.clone()]);
            session.verify_checkouts(&[checkout]).unwrap();
            assert!(session.sealed);
            eprintln!("{version}: graph={shape}, base={base:?}, response_lf={lf}, requests={}, response_bytes={}, elapsed={:?}", session.requests, session.pack_bytes, started.elapsed());
        }
    }
}

#[test]
fn cancellation_and_deadline_remain_latched_in_pack_phase() {
    for cancelled in [false, true] {
        let mut s = advertised();
        let body = pack("side-band-64k");
        fixture(&mut s, 0, false, &body, http(PACK_TYPE, &pack_response()));
        let deadline = if cancelled { until() } else { Instant::now() };
        let expected = if cancelled {
            LocalError::Interrupted
        } else {
            LocalError::Deadline
        };
        assert_eq!(
            s.request(0, false, &body, deadline, &AtomicBool::new(cancelled)),
            Err(expected)
        );
        assert!(s.sealed);
        assert_eq!(s.synthetic.len(), 1);
    }
}
