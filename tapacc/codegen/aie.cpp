#include "aie.h"

#include <string>

#include "clang/AST/Attr.h"
#include "clang/AST/Decl.h"
#include "clang/AST/TypeLoc.h"

#include "frontend/diag.h"

namespace tapa::cc {

void AieBackend::RewriteTaskFunc(const TaskModel& task, bool /*is_top*/,
                                 EditSink& edits) const {
  if (!task.def->hasBody()) return;
  // The signature stub was captured before this runs, so the frame type
  // lands in the task's own variant only; every other variant keeps the
  // portable `tapa::istream<T>&` spelling.
  for (const clang::ParmVarDecl* param : task.def->parameters()) {
    const auto* frame = param->getAttr<clang::TapaAieFrameAttr>();
    const clang::TypeSourceInfo* info = param->getTypeSourceInfo();
    if (frame == nullptr || info == nullptr) continue;
    const auto ref = info->getTypeLoc().getAs<clang::ReferenceTypeLoc>();
    if (ref.isNull()) continue;
    const clang::SourceRange stream = ref.getPointeeLoc().getSourceRange();
    edits.InsertTextBefore(stream.getBegin(), "tapa::aie::frame_t<");
    edits.InsertTextAfterToken(stream.getEnd(),
                               ", " + std::to_string(frame->getSize()) + ">");
  }
}

void AieBackend::LowerPipeline(int /*ii*/, const std::string& /*style*/,
                               const clang::Stmt* body, EditSink& edits) const {
  // chess keywords sit between the loop header and its body.
  if (body != nullptr) {
    edits.InsertTextBefore(body->getBeginLoc(),
                           "chess_prepare_for_pipelining ");
  }
}

void AieBackend::LowerUnroll(int factor, const clang::Stmt* body,
                             EditSink& edits) const {
  if (body == nullptr) return;
  edits.InsertTextBefore(
      body->getBeginLoc(),
      factor == 0 ? std::string("chess_flatten_loop ")
                  : "chess_unroll_loop(" + std::to_string(factor) + ") ");
}

namespace {

void ReportUnsupportedAttr(const clang::Attr& attr, EditSink& edits) {
  ReportCustomDiag(edits.getASTContext(), clang::DiagnosticsEngine::Error,
                   attr.getRange().getBegin(),
                   "[[tapa::%0]] has no AI Engine lowering")
      << attr.getSpelling();
}

}  // namespace

void AieBackend::LowerFuncPipeline(int, const std::string&,
                                   const clang::Stmt* body,
                                   EditSink& edits) const {
  if (body != nullptr) {
    ReportCustomDiag(edits.getASTContext(), clang::DiagnosticsEngine::Error,
                     body->getBeginLoc(),
                     "function-level [[tapa::pipeline]] has no AI Engine "
                     "lowering; put it on the loop");
  }
}

void AieBackend::LowerStmtAttr(const clang::Attr& attr, const clang::Stmt*,
                               EditSink& edits) const {
  ReportUnsupportedAttr(attr, edits);
}

void AieBackend::LowerDeclAttr(const clang::Attr& attr, const clang::VarDecl&,
                               const clang::DeclStmt&, EditSink& edits) const {
  if (attr.getKind() == clang::attr::TapaAieFrame ||
      attr.getKind() == clang::attr::TapaAiePlio) {
    return;
  }
  ReportUnsupportedAttr(attr, edits);
}

void AieBackend::LowerParamAttr(const clang::Attr& attr,
                                const clang::ParmVarDecl&, const clang::Stmt*,
                                EditSink& edits) const {
  if (attr.getKind() == clang::attr::TapaAieFrame ||
      attr.getKind() == clang::attr::TapaAiePlio) {
    return;
  }
  ReportUnsupportedAttr(attr, edits);
}
}  // namespace tapa::cc
