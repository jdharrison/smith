.DEFAULT_GOAL := help

.PHONY: help test build run install client/install service/install installer/test installer/build installer/binary package/test \
	client/test client/build client/run \
	service/test service/build installer/ui installer/ui-test \
	test-service test-client build-service build-client run-client

help:
	@printf '%s\n' \
	  'Run from next/:' \
	  '  make test           Text service/client and isolated installer tests' \
	  '  make build          Export service, client and recipient package' \
		  '  make install        Build; confirm developer artifact installation' \
		  '  make client/install Build and install desktop bundle/menu/command' \
		  '  make service/install Build and install service binary only' \
		  '  make installer/test Native Rust quality, isolated GTK UI and no-interpreter CLI tests' \
	  '' \
	  'Client:' \
	  '  make client/test    Dagger formatting, analysis, and tests' \
	  '  make client/build   Export desktop + service bundle to client/dist/' \
	  '  make client/run     Build helper, then launch the text client locally' \
	  '' \
	  'Service:' \
	  '  make service/test   Dagger formatting, Clippy, and tests' \
	  '  make service/build  Export service/dist/smith-service' \
	  '' \
	  'Recipient package:' \
		  '  make installer/build Export visual + CLI installers and paired payload' \
		  '  make installer/binary Build native tool for component artifact installs' \
		  '  make installer/ui    Export native GTK installer tool' \
		  '  make installer/ui-test Isolated GTK quality and widget smoke' \
		  '  make package/test    Builder-only metadata and native entrypoint tests'

test:
	dagger call test --service service --client client --installer installer --tooling dagger

client/test:
	dagger call test-client --source client --service service

service/test:
	dagger call test-service --source service

installer/test:
	dagger call test-installer --source installer --service service

package/test:
	dagger call test-package --source dagger --installer installer --service service

installer/ui-test:
	dagger call test-installer-ui --source installer --service service

installer/ui:
	dagger call -o installer/build/smith-installer-ui build-installer-ui --source installer --service service

installer/binary:
	dagger call -o installer/build/smith-installer build-installer-binary --source installer --service service

installer/build:
	dagger call -o installer/dist build-installer --installer installer --client client --service service --tooling dagger
	python3 dagger/package.py --root installer/dist --verify

install: service/build client/build installer/binary
	./installer/build/smith-installer --artifacts-only --client client/dist --service service/dist/smith-service

client/install: client/build installer/binary
	./installer/build/smith-installer --artifacts-only --client client/dist

service/install: service/build installer/binary
	./installer/build/smith-installer --artifacts-only --service service/dist/smith-service

build: service/build client/build installer/build

service/build:
	dagger call -o service/dist/smith-service build-service --source service

client/build:
	dagger call -o client/dist build-client --source client --service service

client/run:
	cd service && sh scripts/build.sh
	install -D -m 0755 service/target/release/smith-service service/dist/smith-service
	cd client && flutter run -d linux --dart-define=SMITH_SERVICE_EXECUTABLE=$(abspath service/dist/smith-service)

# Keep the original shortcuts usable while the namespaced names become primary.
run run-client: client/run
test-service: service/test
test-client: client/test
build-service: service/build
build-client: client/build
