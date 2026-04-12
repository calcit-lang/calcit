
{} (:about "|Machine-generated snapshot. AI AGENTS: never edit this file directly — changes will be overwritten on recompile. Inspect via `cr query`; modify via `cr edit` / `cr tree`. MANDATORY first step: run `cr docs agents --full`.") (:package |test-method-errors)
  :configs $ {} (:init-fn |test-method-errors.main/main!) (:reload-fn |test-method-errors.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |test-method-errors.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc "|Entry for reproducing preprocess failures") (:schema nil)
          :code $ quote
            defn main! () (; "运行该入口会在" preprocess "阶段报错，验证类型推断是否生效") (trigger-type-error)
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler") (:schema nil)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
        |trigger-type-error $ %{} :CodeEntry (:doc "|Pipeline sample that should fail preprocess type checks") (:schema nil)
          :code $ quote
            defn trigger-type-error () $ let
                src $ {} (:a 1) (:b 2)
                by-set $ .to-set (vals src)
              .map by-set $ fn (x) false
          :examples $ []
      :ns $ %{} :NsEntry (:doc "|Namespace for standalone repro")
        :code $ quote (ns test-method-errors.main)
