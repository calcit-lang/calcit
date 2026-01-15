{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |compute $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn compute (x) :number
              &+ x 10

        |test-nested-scope $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-nested-scope ()
              ; 测试：外层定义的变量可以被内层使用，并保留类型信息
              let
                  a 100
                let
                    ; b 使用外层的 a，a 的类型信息应该传递进来
                    b $ &+ a 20
                  let
                      ; c 使用中层的 b 和外层的 a
                      c $ &+ b a
                      ; d 调用函数，函数的返回类型应该推断出来
                      d $ compute c
                    ; 最终返回 d，类型应该是 :number
                    d

        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! ()
              println $ test-nested-scope

        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote (defn reload! () nil)

      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote (ns app.main)
