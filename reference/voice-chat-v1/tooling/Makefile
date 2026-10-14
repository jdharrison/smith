.DEFAULT_GOAL := help

.PHONY: help test build run install client/install service/install installer/test services/test \
	client/test client/build client/run \
	service/test service/build \
	test-service test-client build-service build-client run-client

help:
	@printf '%s\n' \
	  'Run from next/:' \
	  '  make test           Component, installer and voice-provider tests' \
	  '  make build          Export both component builds to their dist/' \
		  '  make install        Build both; confirm user artifact installation' \
		  '  make client/install Build and install desktop bundle/menu/command' \
		  '  make service/install Build and install service binary only' \
		  '  make installer/test Dagger isolated installer tests' \
	  '' \
	  'Client:' \
	  '  make client/test    Dagger formatting, analysis, and tests' \
	  '  make client/build   Export desktop + service bundle to client/dist/' \
	  '  make client/run     Build helper, then launch the debug client locally' \
	  '' \
	  'Service:' \
	  '  make service/test   Dagger formatting, Clippy, and tests' \
	  '  make service/build  Export service/dist/smith-service' \
		  '' \
		  'Voice services:' \
		  '  make services/test  Build speech image and run isolated fake tests'

test:
	dagger call test --service service --client client --installer dagger --sidecars sidecars

client/test:
	dagger call test-client --source client --service service

service/test:
	dagger call test-service --source service

installer/test:
	dagger call test-installer --source dagger

services/test:
	dagger call test-sidecars --source sidecars

install: build
	python3 dagger/install.py --client client/dist --service service/dist/smith-service

client/install: client/build
	python3 dagger/install.py --client client/dist

service/install: service/build
	python3 dagger/install.py --service service/dist/smith-service

build: service/build client/build

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
