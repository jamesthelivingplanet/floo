"""Core registry behaviors. Each test gets a fresh DB via tmp_path."""
from __future__ import annotations

from pathlib import Path

import pytest

from floo import registry


@pytest.fixture
def db(tmp_path: Path, monkeypatch):
    # Redirect the registry to a per-test DB.
    monkeypatch.setattr(registry, "db_path", lambda: tmp_path / "registry.db")
    yield


def test_claim_is_idempotent(db):
    r1 = registry.claim("/repo/A", "web")
    r2 = registry.claim("/repo/A", "web")
    assert r1.claim.port == r2.claim.port
    assert r1.was_new is True
    assert r2.was_new is False


def test_distinct_repo_and_service_get_distinct_ports(db):
    p_a = registry.claim("/repo/A", "web").claim.port
    p_b = registry.claim("/repo/B", "web").claim.port
    p_c = registry.claim("/repo/A", "storybook").claim.port
    assert len({p_a, p_b, p_c}) == 3


def test_release_then_reclaim(db):
    p1 = registry.claim("/repo/A", "web").claim.port
    assert registry.release("/repo/A", "web") is True
    # After release the row is gone; a fresh claim starts the allocator over.
    r2 = registry.claim("/repo/A", "web")
    assert r2.was_new is True
    # Same port might or might not come back depending on what else is taken;
    # we just assert the claim succeeded.
    assert isinstance(r2.claim.port, int)


def test_release_all(db):
    registry.claim("/repo/A", "web")
    registry.claim("/repo/B", "web")
    assert registry.release_all() == 2
    assert registry.list_claims() == []


def test_gc_reclaims_never_seen(db):
    registry.claim("/repo/A", "web")
    # Any positive offset, since we just created the row and last_seen is NULL,
    # so we fall through to the created_at branch. Use -0 seconds to force
    # immediate eligibility.
    cands = registry.gc(older_than="-0 seconds")
    assert len(cands) == 1
    assert cands[0].claim.service == "web"
    assert registry.list_claims() == []
