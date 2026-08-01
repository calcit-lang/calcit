
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-method-errors) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-method-errors.main/main!) (:mode :native) (:reload-fn 'test-method-errors.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-method-errors.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc "|Entry for reproducing preprocess failures")
          :code $ quote
            defn main! () (; "运行该入口会在" preprocess "阶段报错，验证类型推断是否生效") (trigger-type-error)
          :examples $ []
          :schema $ :: 'Dynamic
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () $ :: 'Unit
          :examples $ []
          :schema $ :: 'Dynamic
        |trigger-type-error $ %{} :CodeEntry (:doc "|Pipeline sample that should fail preprocess type checks")
          :code $ quote
            defn trigger-type-error () $ let
                src $ {} (:a 1) (:b 2)
                by-set $ .to-set (vals src)
              .map by-set $ fn (x) false
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} :NsEntry (:doc "|Namespace for standalone repro")
        :code $ quote (ns test-method-errors.main)
