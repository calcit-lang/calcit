
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
  :files $ {}
    |app.main $ {}
      :ns $ quote
        ns app.main $ :require
      :defs $ {}
        |main! $ quote
          defn main! ()
            slurp-cirru-edn |/Users/chen/repo/calcit-lang/apis/docs/apis.cirru

        |reload! $ quote
          defn reload! ()
            echo "|TODO"

        |slurp-cirru-edn $ quote
          defmacro slurp-cirru-edn (file)
            with-cpu-time $ stringify-json
              first $ with-cpu-time (parse-cirru $ read-file file)
              , true

      :proc $ quote ()
      :configs $ {} (:extension nil)
