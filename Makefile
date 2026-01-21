SHELL := /bin/bash

ifneq (,$(wildcard .env))
  include .env
  export
endif

.PHONY: help
help:
	@echo "Targets:"
	@echo "  make push          - Git: Added, commit and push"


.PHONY: push
push:
	@read -p "Commit message: " msg; \
	cargo clean && git add . && git commit -m "$$msg" && git push
