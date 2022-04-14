

{} (:package |util)
  :configs $ {} (:init-fn |util.core/main!) (:reload-fn |util.core/reload!)
  :files $ {}
    |util.core $ {}
      :ns $ quote
        ns util.core $ :require
      :defs $ {}

        |log-title $ quote
          defn log-title (title)
            println
            println title
            println

        |inside-eval: $ quote
          defmacro inside-eval: (& body)
            if
              = :eval $ &get-calcit-running-mode
              quasiquote
                do (println "|env: eval") ~@body
              quasiquote
                do (println "|env: not eval. tests skipped")

        |inside-js: $ quote
          defmacro inside-js: (& body)
            if
              not= :eval $ &get-calcit-running-mode
              quasiquote
                do (println "|env: js") ~@body
              quasiquote
                do (println "|env: not js. tests skipped")

        |main! $ quote
          defn main! () nil

        |reload! $ quote
          defn reload! () nil

      :proc $ quote ()
      :configs $ {} (:extension nil)
