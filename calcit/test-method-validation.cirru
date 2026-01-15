{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |test-valid-list-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-valid-list-methods ()
              let
                  xs $ [] 1 2 3
                assert-type xs :list
                ; 合法的 list 方法
                .first xs

        |test-invalid-list-method $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-invalid-list-method ()
              let
                  xs $ [] 1 2 3
                assert-type xs :list
                ; 非法：list 没有 invalid-method
                .invalid-method xs

        |test-invalid-string-method $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-invalid-string-method ()
              let
                  text |hello
                assert-type text :string
                ; 非法：string 没有 invalid-string-method
                .invalid-string-method text

        |test-invalid-map-method $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-invalid-map-method ()
              let
                  m $ {} (:a 1)
                assert-type m :map
                ; 非法：map 没有 invalid-map-method
                .invalid-map-method m

        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              test-valid-list-methods
              ; test-invalid-list-method  ; 会导致 preprocess 错误
              ; test-invalid-string-method  ; 会导致 preprocess 错误
              test-invalid-map-method  ; 测试 map 方法验证
              println |All tests passed

        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)

      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns app.main)
