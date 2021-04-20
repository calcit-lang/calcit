

{} (:package |util)
  :configs $ {} (:init-fn |util.core/main!) (:reload-fn |util.core/reload!)
  :files $ {}
    |util.core $ {}
      :ns $ quote
        ns util.core $ :require
      :defs $ {}

        |log-title $ quote
          defn log-title (title)
            echo
            echo title
            echo

        |inside-nim: $ quote
          defmacro inside-nim: (& body)
            if
              = :eval $ &get-calcit-running-mode
              quote-replace
                do (echo "|env: eval") ~@body
              quote-replace
                do (echo "|env: not eval. tests skipped")

        |inside-js: $ quote
          defmacro inside-nim: (& body)
            if
              not= :eval $ &get-calcit-running-mode
              quote-replace
                do (echo "|env: js") ~@body
              quote-replace
                do (echo "|env: not js. tests skipped")

        |main! $ quote
          defn main! () nil

        |reload! $ quote
          defn reload! () nil

      :proc $ quote ()
      :configs $ {} (:extension nil)
