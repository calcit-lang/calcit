{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |test-map-error $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-map-error ()
              let
                  xs $ {} (:a 1)
                assert-type xs :map
                ; 错误：list 没有 bad-method 方法
                .bad-method xs

        |test-map-error $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-map-error ()
              let
                  text {} (:a 1)
                assert-type text :map
                ; 错误：string 没有 bad-method 方法
                .bad-method text

        |test-map-error $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-map-error ()
              let
                  m $ {} (:a 1)
                assert-type m :map
                ; 错误：map 没有 invalid-map-method 方法
                .invalid-map-method m

        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              ; 直接在 main 中测试，确保 preprocess 能检查到
              let
                  xs $ {} (:a 1)
                assert-type xs :map
                .bad-method xs

        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)

      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns app.main)
