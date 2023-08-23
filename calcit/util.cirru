
{} (:package |util)
  :configs $ {} (:init-fn |util.core/main!) (:reload-fn |util.core/reload!)
  :files $ {}
    |util.core $ {}
      :configs $ {}
      :defs $ {}
        |inside-eval: $ %{} :CodeEntry
          :code $ quote
            defmacro inside-eval: (& body)
              if
                = :eval $ &get-calcit-running-mode
                quasiquote $ do (println "|env: eval") ~@body
                quasiquote $ do (println "|env: not eval. tests skipped")
          :doc |
        |inside-js: $ %{} :CodeEntry
          :code $ quote
            defmacro inside-js: (& body)
              if
                not= :eval $ &get-calcit-running-mode
                quasiquote $ do (println "|env: js") ~@body
                quasiquote $ do (println "|env: not js. tests skipped")
          :doc |
        |log-title $ %{} :CodeEntry
          :code $ quote
            defn log-title (title) (println) (println title) (println)
          :doc |
        |main! $ %{} :CodeEntry
          :code $ quote
            defn main! () $ :: :unit
          :doc |
        |reload! $ %{} :CodeEntry
          :code $ quote
            defn reload! () $ :: :unit
          :doc |
      :ns $ %{} :CodeEntry
        :code $ quote
          ns util.core $ :require
        :doc |
      :proc $ quote ()
