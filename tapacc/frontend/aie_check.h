#ifndef TAPA_FRONTEND_AIE_CHECK_H_
#define TAPA_FRONTEND_AIE_CHECK_H_

#include <set>
#include <string>

#include "clang/AST/ASTContext.h"
#include "clang/AST/Decl.h"

#include "program.h"

namespace tapa::cc {

// Reads `[[tapa::aie_frame(N)]]` / `[[tapa::aie_plio(W)]]` onto the task's
// stream ports and `[[tapa::aie_location(...)]]` onto the task, reporting
// an error for a knob on a task that is not on the AI Engine, on the wrong
// kind of port, or with an out-of-range value.
void ReadAieAttrs(const clang::ASTContext& ctx, TaskModel& task);

// The AI Engine eligibility rules for one AI Engine task: no memory ports,
// stream elements a multiple of 32 bits, and (for a leaf) only the blocking
// stream operations -- AI Engine streams carry no end-of-transmission token
// and expose no occupancy. Each violation is an error at its source site.
void CheckAieTask(const clang::ASTContext& ctx, const TaskModel& task);

// The stream ports a leaf body closes (`close()` / `try_close()`).
std::set<std::string> ClosedPorts(const clang::FunctionDecl* def);

}  // namespace tapa::cc

#endif  // TAPA_FRONTEND_AIE_CHECK_H_
