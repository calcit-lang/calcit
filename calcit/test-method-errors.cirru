{} (:package |test-method-errors)
  :configs $ {} (:init-fn |test-method-errors.main/main!) (:reload-fn |test-method-errors.main/reload!)
  :files $ {}
    |test-method-errors.main $ %{} :FileEntry
      :defs $ {}
        |trigger-type-error $ %{} :CodeEntry (:doc "|Pipeline sample that should fail preprocess type checks")
          :code $ quote
            defn trigger-type-error ()
              let
                  src $ {} (:a 1) (:b 2)
                  by-set $ .to-set $ vals src
                .map by-set
                  fn (x) false

        |main! $ %{} :CodeEntry (:doc "|Entry for reproducing preprocess failures")
          :code $ quote
            defn main! ()
              ; 运行该入口会在 preprocess 阶段报错，验证类型推断是否生效
              trigger-type-error

        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () $ :: :unit

      :ns $ %{} :CodeEntry (:doc "|Namespace for standalone repro")
        :code $ quote
          ns test-method-errors.main
