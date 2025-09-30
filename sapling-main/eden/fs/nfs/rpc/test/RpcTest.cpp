/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

#include "eden/fs/nfs/rpc/Rpc.h"

#include <gtest/gtest.h>

#include "eden/fs/nfs/testharness/XdrTestUtils.h"

namespace {

using namespace facebook::eden;

TEST(RpcTest, enums) {
  roundtrip(auth_flavor::AUTH_NONE);
  roundtrip(opaque_auth{});

  roundtrip(rejected_reply{{reject_stat::RPC_MISMATCH, mismatch_info{0, 1}}});
  roundtrip(rejected_reply{{reject_stat::AUTH_ERROR, auth_stat::AUTH_FAILED}});
}

} // namespace
