/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

#pragma once

#ifndef _WIN32
#include "eden/fs/fuse/FuseDispatcher.h"
#else
#include "eden/fs/prjfs/PrjfsDispatcher.h"
#endif

#include "eden/fs/nfs/NfsDispatcher.h"

namespace facebook::eden {

class EdenMount;

class EdenDispatcherFactory {
 public:
#ifndef _WIN32
  static std::unique_ptr<FuseDispatcher> makeFuseDispatcher(EdenMount* mount);
#else
  static std::unique_ptr<PrjfsDispatcher> makePrjfsDispatcher(EdenMount* mount);
#endif
  static std::unique_ptr<NfsDispatcher> makeNfsDispatcher(EdenMount* mount);
};

} // namespace facebook::eden
